# PLAN: engine README як перевірені doctests (варіант A-уніф. через doc-macros)

Мета: зробити `mqtt-topic-engine/README.md` крейт-рівневою документацією через
`include_md_transformed!` (як у головному крейті), щоб приклади з README **залишились у README**
і водночас компілювалися/перевірялися (`cargo test --doc`). Без зміни поведінки бібліотеки.

Рішення користувача: A-уніфікована **через `doc-macros`** (повна консистентність із головним крейтом),
приймаємо coupling engine→`mqtt-typed-client-doc-macros` на crates.io.

---

## Контекст / факти (звірено з кодом)
- `include_md_transformed!(path, transform="readme")`: читає файл від `CARGO_MANIFEST_DIR`
  (для engine → `mqtt-topic-engine/README.md`); `readme`-трансформація робить ЛИШЕ заміну
  `[examples/](examples/)` → `` [`crate::examples`] `` (у engine такого лінка немає → no-op) і
  додає **невидимий** HTML-коментар-хедер "AUTO-GENERATED…". Хедер — це `<!-- -->`, тож у
  рендері docs.rs його не видно (лише у "view source"). README на диску НЕ змінюється — макрос
  інлайнить контент під час компіляції.
- Головний крейт (`src/lib.rs`): `//! # MQTT Typed Client` + `#![doc = …include_md_transformed!…]`
  + далі `//!`-секції. Блоки README позначає `rust,no_run` / `rust,ignore`.
- `doc-macros` уже в `[workspace.dependencies]` як `mqtt-typed-client-doc-macros =
  { path="./doc-macros", version="0.1.0" }`.
- У engine `rumqttc`/`paho-mqtt`/`ntex-mqtt` — **опційні** фічі; під дефолтними (`router`,`lru-cache`)
  їх немає. `cargo test --doc` ганяється з дефолтними фічами → блоки з `rumqttc::`/`paho_mqtt::`/
  `ntex_mqtt::` під дефолтом НЕ скомпілюються.
- `no_run` doctest КОМПІЛЮЄТЬСЯ і ЛІНКУЄТЬСЯ (лише не виконується) → paho-блок як `no_run` впаде на
  лінкуванні `paho-mqtt3a-static.lib`. Тому paho/ntex/rumqttc-приклади → `ignore` (не компілюються),
  АБО переписати на власний `QoS`.
- Поточний стан doctests: 5 (2 pass + 3 ignore — це `ignore`-приклади в `qos.rs`).
- Engine README: 19 ```rust-блоків (рядки 73,91,113,150,185,211,233,256,275,305,328,371,427,446,
  469,488,514,547,620) + toml/text-блоки (їх не чіпаємо).

---

## Корективи після рев'ю критика (ОБОВ'ЯЗКОВІ)
- **Б1:** `doc-macros` НЕ в кореневому `[workspace.dependencies]` (це залежність кореневого ПАКЕТА).
  Тож `{ workspace = true }` НЕ скомпілюється. Використати пряму path-залежність:
  `mqtt-typed-client-doc-macros = { path = "../doc-macros", version = "0.1.0" }`.
- **В2:** голі URL у README (рядки 660-661 `http://...`) дають `rustdoc::bare_urls` warning →
  обгорнути в `<...>`, інакше критерій "0 warnings" не виконується.
- **В1:** блок Cache Strategies (514) має `#[cfg(feature="lru-cache")]` ВСЕРЕДИНІ doctest — cfg
  рахується проти крейта-тесту, рядок мовчки викидається (хибне покриття). Прибрати cfg-рядки
  (lru-cache у дефолті) — інакше не вважати перевіреним.
- **В4:** no_run-блоки з невикористаними змінними (514, 547, 469, 488) дають unused-warnings →
  приховані `# let _ = …;`.
- **Drift:** блок 620 має `let paho_qos: i32 = engine_qos.to_paho_mqtt();`, але метод повертає
  `paho_mqtt::QoS` — виправити (блок лишається `ignore`).

## Розмітка 19 блоків (рішення)
- **RUN** (plain ```rust, з `# fn main()`-обгорткою якщо є `?`) — мають assert'и, перевіряють поведінку:
  233 (validation), 305 (wildcard).
- **no_run** (компіляція без виконання, з прихованою обгорткою) — чистий engine-API:
  73, 91, 113, 150, 185, 211, 256, 275, 328, 427, 446, 469, 488, 514, 547.
- **ignore** — потребує опційних фіч/нативу/повного async-app: 371 (rumqttc+tokio+broker), 620 (paho/ntex cfg).
- **Переписати** `use rumqttc::QoS;` → `use mqtt_topic_engine::QoS;` у 150/275/328 (роутер бере власний QoS;
  критик підтвердив коректність). Блок 371 лишається `ignore` із rumqttc (це і є rumqttc-інтеграція).

## 2 НОВІ приклади (варіант «б» — розкрити цінність роутера)
Додати дві нові ```rust-секції (RUN, з assert'ами, на власному `QoS` — без брокера):
- **QoS Aggregation** (після «Managing Subscriptions»): додати ОДИН патерн двічі — AtMostOnce →
  `needs_subscribe=true`; той самий AtLeastOnce (вищий) → `true`; знову AtMostOnce → `false`.
  assert'и на ці три значення. Пояснити: роутер тримає МАКС QoS, ресабскрайб лише при підвищенні.
- **Reconnect: resubscribe** (після «Complex Routing»): кілька перекривних підписок із різним QoS,
  тоді `get_topics_for_resubscribe()` → `HashMap<ArcStr,QoS>` із дедуплікованими патернами й
  агрегованим МАКС QoS; loop «client.subscribe» при реконнекті. assert на агрегований QoS.

## Зміни

### 1. Залежність
`mqtt-topic-engine/Cargo.toml` → у `[dependencies]` додати (path, НЕ workspace — див. Б1):
```toml
mqtt-typed-client-doc-macros = { path = "../doc-macros", version = "0.1.0" }
```
Не optional. Наслідок для публікації: `doc-macros` (0.1.0) публікується ПЕРЕД engine (вже в порядку).

### 2. lib.rs — підключити README як крейт-док (дзеркало головного крейта)
`mqtt-topic-engine/src/lib.rs` — замінити поточний 4-рядковий `//!`-блерб на патерн root:
```rust
//! # MQTT Topic Engine
//!
#![doc = mqtt_typed_client_doc_macros::include_md_transformed!("README.md", transform = "readme")]

#![warn(missing_docs)]
```
(`#![warn(missing_docs)]` лишається після; inner-attrs ідуть до items — валідно.)
Перевірити: чи README не дає подвійний H1 (README сам стартує з `# MQTT Topic Engine`).
Якщо дубль H1 негарний — лишити лише макрос без `//! # …` рядка. РІШЕННЯ при імплементації за
фактичним рендером; дефолт — як у root (короткий title + include).

### 3. README — розмітити всі 19 rust-блоків
Принцип: **максимум перевіряти** (це і була мета A), інтеграційне (потребує опційних фіч/нативу) — `ignore`.

**Категорія REAL (`rust,no_run`) — чистий engine-API під дефолтними фічами:**
для блоків, що використовують `?`, додати приховані рядки `# use …;` + обгортку
`# fn main() -> Result<(), Box<dyn std::error::Error>> {` … `# Ok(()) }`.
Кандидати (звірити при імплементації, що компілюються):
- 73 Basic Pattern Matching, 91 Named Parameters, 113 Multi-Level Wildcards
- 185 Parameter Binding
- 211 LRU Caching (потребує `lru-cache` — є в дефолті)
- 233 Validation, 256 Building Topics, 275 Managing Subscriptions, 305 Wildcard
- 488 ArcStr, 514 Cache Strategies, 547 Minimal Config
- 150 Topic Router — **переписати** `use rumqttc::QoS;` → `use mqtt_topic_engine::QoS;`
  (роутер бере `impl Into<QoS>`; це робить блок чистим тестом без опційних фіч).
  ⚠️ Зміна тексту README-прикладу — узгодити, що семантика збережена.

**Категорія IGNORE (`rust,ignore`) — потребує опційних фіч / нативного лінку / псевдокод:**
- 620 QoS Type Conversions (paho/ntex/rumqttc) — `ignore` (paho лінкує C-lib).
- 371 Integration with MQTT Clients, 328 Complex Routing, 427/446/469 Use Cases —
  оцінити: якщо чистий engine-API → REAL; якщо тягне зовнішні клієнти/довгий app-код → `ignore`.
  Рішення поблочно при імплементації; кожен лишений `ignore` — залогувати (нижче).

**Не чіпати:** toml/text/bash блоки (31,40,542,600 …).

> ⚠️ NO SILENT CAPS: у кінці імплементації — список, які блоки стали `no_run` (перевіряються),
> а які лишились `ignore` (і чому), щоб не створювати ілюзію "перевірено все".

### 4. CHANGELOG / TODO
- `mqtt-topic-engine/CHANGELOG.md` (Unreleased або в 0.1.0): рядок "README examples are now
  compiled as doctests; crate docs include the README".
- `.github/dev-docs/TODO-0.2.0-release.md`: відмітити цей під-пункт (README/приклади engine).

---

## Відкриті питання для критика
1. Чи `transform="readme"` доречний для engine, чи краще без трансформації
   (`include_md_transformed!("README.md")`) — щоб не додавати оманливий "AUTO-GENERATED/DO NOT EDIT"
   хедер (хай і невидимий)? Чи взагалі обрати plain `#![doc = include_str!("README.md")]` без
   doc-macros? (Користувач обрав doc-macros — але чи є технічна причина проти?)
2. Подвійний H1 (`//! # MQTT Topic Engine` + README `# MQTT Topic Engine`) — як рендериться на
   docs.rs, чи треба прибрати один?
3. Чи `cargo test --doc` для engine реально ганяється з дефолтними фічами, і чи `no_run`-блоки
   з `lru-cache`/`router` API компілюються під дефолтом? Чи треба явно вказувати фічі?
4. Чи переписування router-прикладу з `rumqttc::QoS` на `mqtt_topic_engine::QoS` не вводить в оману
   (README мав показати саме інтеграцію з rumqttc)? Може лишити `ignore` з `rumqttc::QoS`?
5. relative-лінки в README (`[LICENSE-MIT](LICENSE-MIT)`, `[mqtt-typed-client](https://…)`): як вони
   поведуться як крейт-док на docs.rs (відносні → можливі 404)? Чи `readme`-трансформація це лагодить
   (ні — лише `[examples/]`)? Чи це блокер?
6. Чи додавання non-optional dep на `doc-macros` не ламає матрицю фіч / `--no-default-features`
   білд engine (doc-macros потрібен лише для doku, але буде в deps завжди)?

## Перевірка (після імплементації)
- `cargo test -p mqtt-topic-engine --doc` — зелене (нові no_run компілюються; жодного впалого).
- `cargo build -p mqtt-topic-engine --no-default-features` — ОК (doc-macros не ламає).
- `cargo doc -p mqtt-topic-engine --all-features --no-deps` — 0 warning (missing_docs + broken-intra-doc).
- `cargo build --workspace` — root та інші члени не зачеплені.
- `cargo clippy -p mqtt-topic-engine --all-targets` — чисто.
- Перелік: скільки блоків `no_run` (перевірено) vs `ignore` (і чому).

## Коміт
Один коміт (hook). Повідомлення — секція про підключення README як doctests + залежність doc-macros.
Co-Authored-By trailer.
