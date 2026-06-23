# План: тести + дедуплікація per-topic serializer (одна сесія)

Мета: закрити нульове тестове покриття фічі `serializer = Type`, потім безпечно прибрати
дублювання в `codegen.rs`, і (опц.) додати живий round-trip тест.

## Факти про поточний стан (звірено з кодом)
- `MacroArgs { pattern, generate_subscriber, generate_publisher, generate_typed_client,
  generate_last_will, custom_serializer: Option<syn::Type> }` (`macros/src/lib.rs`).
- `macros/src/codegen_test.rs`: інфраструктура є — `create_macro_args(pattern, config)` **хардкодить
  `custom_serializer: None`**. Тести стрінгіфікують TokenStream і роблять `.contains()` через
  enum `CodeCheck` (Method / NotPresent / PayloadType / FormatString / ...). Хелпер
  `run_codegen_test` викликає `generate_complete_implementation`.
- Дублювання у `codegen.rs` — три методи мають гілки `if custom_serializer.is_some() {…} else {…}`:
  - `generate_builder_methods` (рядки 148-207): `subscription<F>` — різниця лише в типі результату
    (`SubscriptionBuilder<Self, #S>` vs `<Self, F>`), bound'ах і `client.clone_with_serializer::<#S>()`
    vs `client.clone()`. `default_pattern()` у обох гілках ІДЕНТИЧНИЙ (чистий копіпаст).
  - `generate_subscriber_methods` (220-269): `subscribe<F>` — різниця в типі `MqttTopicSubscriber<…, #S>`
    vs `<…, F>` і where-bounds (`F: Clone, #S: …+'static+MessageSerializer` vs `F: …+MessageSerializer`).
  - `generate_publisher_methods` (272-403): `publish`/`get_publisher`/`get_publisher_to`:
    serializer-гілка дістає клієнта через `client.clone_with_serializer::<#S>().get_publisher(...)`
    (рядки 313,340), None-гілка — **голий `client.get_publisher(...)`** (рядки 374,400, БЕЗ `.clone()`).
    Тобто «спосіб дістати serializer-aware client» має ТРИ форми: builder = `clone_with_serializer::<S>()`
    vs `client.clone()` (173 vs 201); publisher = `clone_with_serializer::<S>()` vs голий `client`.
- При `custom_serializer.is_some()` → `should_generate_typed_client()` = false (TypedClient ext НЕ
  генерується). Це треба перевірити тестом (NotPresent).
- `clone_with_serializer` згенерується ЛИШЕ у serializer-гілці — гарний маркер для `.contains()`.

---

## Крок 1. Офлайн codegen-тест на `serializer = Type` (СПЕРШУ — страхувальна сітка)

1a. Додати хелпер у `codegen_test.rs`:
```rust
fn create_macro_args_with_serializer(
    pattern: &str, config: GenerationConfig, serializer: syn::Type,
) -> MacroArgs { /* як create_macro_args, але custom_serializer: Some(serializer) */ }
```
   Рефакторити `create_macro_args` так, щоб не дублювати (наприклад внутрішній
   `create_macro_args_inner(pattern, config, custom_serializer)`).
   Також додати варіант `create_generator_with_serializer(...)`.

1b. Додати тест `test_custom_serializer_generation` (`GenerationConfig::Both`,
    `serializer: parse_quote!(JsonSerializer)`), що перевіряє згенерований код:
   - присутній `clone_with_serializer` (точна форма з пробілами TokenStream:
     `clone_with_serializer :: < JsonSerializer >` — звірити фактичний стрінгіфай);
   - тип серіалізатора `JsonSerializer` присутній у сигнатурах
     (`MqttTopicSubscriber < … JsonSerializer >`, `MqttPublisher < … JsonSerializer >`,
     `SubscriptionBuilder < … JsonSerializer >`);
   - є методи `subscribe`, `publish`, `get_publisher`, `subscription`;
   - TypedClient НЕ генерується. ⚠️ ПРАВИЛЬНІ маркери (звірено з `naming.rs:21-22`):
     ext-трейт = `{Struct}Ext` (для `TestStruct` → `TestStructExt`), а typed-client struct =
     `{Struct}Client` (→ `TestStructClient`). Перевіряти `NotPresent("TestStructClient")`
     (найнадійніший — це struct typed-client'а) і `NotPresent("TestStructExt")`.
     **НЕ** `TestStructClientExt` — такого рядка не існує, перевірка була б фальшиво-зеленою.
     УВАГА: трейт `TestStructSubscriptionBuilderExt` (`codegen.rs:476`) ОЧІКУЄТЬСЯ присутнім при
     subscriber — це НЕ typed-client; не «посилювати» NotPresent до підрядка `Ext`.
   ⚠️ Точні рядки `.contains()` визначити емпірично: спершу згенерувати, надрукувати
   `code` (як роблять наявні assert-меседжі), підігнати під реальний стрінгіфай TokenStream
   (пробіли навколо `::`, `<`, `>`). Тест лише стрінгіфаїть TokenStream і НЕ компілює згенерований
   код, тому `JsonSerializer` НЕ мусить бути в scope macros-крейта (`parse_quote!(JsonSerializer)`
   дає валідний `syn::Type::Path` з простого ідентифікатора).

1c. (опц.) `NotPresent`-перевірка, що у serializer-гілці НЕ використовується голий
    `client.clone()` без `_with_serializer` там, де очікуємо S — обережно, бо підрядок
    `clone` є і в `clone_with_serializer`. Краще не робити крихких перевірок.

1d. Прогнати `cargo test -p mqtt-typed-client-macros`. Тест має пройти ДО рефактора.

> Примітка: `trybuild` compile-pass — НЕ робимо в цьому кроці (потребує нову dev-залежність і
> fixtures). Приклад `102_multi_serializer_macro.rs` уже дає компіляційне покриття через macro.
> Лишити trybuild як опційний пункт, якщо захочемо строгішого compile-fail покриття.

---

## Крок 2. Дедуплікація `codegen.rs` (ТІЛЬКИ після зеленого Кроку 1)

Підхід: обчислити різницю між гілками один раз, шаблонізувати спільну форму через `quote!`.

⚠️ Реалістична оцінка виграшу: where-bounds між гілками асиметричні — змінюється і набір
предикатів, і змінна, на яку вони навішані (None-subscriber: `F: Default+Clone+Send+Sync+MessageSerializer`
БЕЗ `'static`; serializer-subscriber: `F: Clone, #S: Default+Clone+Send+Sync+'static+MessageSerializer`).
Тому дедуп НЕ зводиться до простої підстановки типу: спільним реалістично лишиться **тіло методів +
`default_pattern`**, а generics+where доведеться будувати per-гілка (окремий хелпер, що повертає
`(generics, where_clause)` для кожного з трьох випадків). Очікувати скромніший виграш, ніж «-120 рядків».

2a. Виділити в `CodeGenerator` приватний хелпер, що повертає компоненти серіалізатора:
```rust
// повертає (serializer_ty, client_expr_with_serializer, builder_serializer_ty)
```
   Конкретно для кожного методу різниця зводиться до:
   - `serializer_ty: TokenStream` = `quote!(#S)` або `quote!(F)`;
   - спосіб отримати serializer-aware client:
     `quote!(client.clone_with_serializer::<#S>())` vs `quote!(client.clone())` (builder)
     / `quote!(client)` (publisher get_publisher);
   - where-bounds — РІЗНІ за структурою (`F: Clone, #S: …+'static+…` vs `F: …+…`), тож
     зробити окремий хелпер, що повертає `generics` + `where_clause` TokenStream для кожного
     з трьох випадків (builder/subscriber/publisher).

2b. Переписати `generate_builder_methods`, `generate_subscriber_methods`,
    `generate_publisher_methods` так, щоб тіло `quote!` було ОДНЕ, а відмінності — підставлялись
    через обчислені токени. `default_pattern()` у builder винести з гілок (він ідентичний).

2c. Прогнати `cargo test -p mqtt-typed-client-macros` — усі старі тести (None-гілка) + новий
    тест (Some-гілка) мають лишитись зеленими. Це і є сенс Кроку 1.

2d. `cargo build --workspace --all-targets` + `cargo build --example 102_multi_serializer_macro`
    (компіляційна перевірка реального користувацького коду).

⚠️ Ризик: bounds легко переплутати (`'static`, `Clone` на `F`). Якщо дедуп робить код менш
читабельним або крихким — зупинитись на частковій дедуплікації (винести лише `default_pattern`
і spсільні шматки), не форсувати «ідеальний» один шаблон.

---

## Крок 3 (опц.). Інтеграційний тест per-topic serializer через брокер

3a. Новий тест-файл (або секція) на кшталт `tests/serializers_integration.rs`, але через макрос:
   оголосити `#[mqtt_topic("test/serializer/{id}", serializer = JsonSerializer)]` структуру,
   під'єднатись до `mqtt://localhost:1883`, publish→subscribe→deserialize round-trip.
3b. Має ТИХО деградувати без брокера (як решта інтеграційних тестів) — connection failure = OK.
3c. Прогнати з піднятим EMQX (порт 1883 уже відкритий у цій сесії).
3d. ⚠️ `JsonSerializer` під `#[cfg(feature="json")]`. `json` є в default features core, тож
   `cargo test`/`cargo build --example 102` за замовчуванням ОК. Але під `--no-default-features`
   і цей тест, і приклад 102 не зберуться — врахувати при налаштуванні CI.

---

## Порядок виконання та верифікація
1. Крок 1 → `cargo test -p mqtt-typed-client-macros` зелено.
2. Крок 2 → ті самі тести зелено + build all-targets + example 102.
3. Крок 3 (опц.) → тест з брокером зелено.
4. Фінал: `cargo test --workspace`, `cargo clippy -p mqtt-typed-client-macros`.
5. Документацію НЕ чіпаємо (за рішенням — у фінал).

## Поза обсягом
- README/CHANGELOG (документація — окремий фінальний блок).
- generic-серіалізатори `S<T>` у парсері (низький пріоритет).
- trybuild compile-fail тести.
