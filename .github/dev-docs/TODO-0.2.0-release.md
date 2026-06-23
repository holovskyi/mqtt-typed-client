# TODO: довести бібліотеку до релізу 0.2.0

Мета: повна і якісна імплементація + документація фіч, які вимагали користувачі (Reddit-фідбек),
і реліз-гігієна для публікації на crates.io.

Статус контексту:
- API backward-compat shim для v0.1.0 шляхів — ✅ зроблено (коміт 1c31bc8).
- Version bump → 0.2.0 (core/macros/root); engine + doc-macros лишаються 0.1.0 — ✅ зроблено.
- Брокер для інтеграційних тестів: `dev/docker-compose.yml` (EMQX, localhost:1883). Тести
  деградують тихо без брокера.

---

## 🔴 Високий пріоритет — без цього фічі не «готові» для користувача

### Міграція rumqttc 0.24 → 0.25.1 (закриває скаргу про aws-lc)
Контекст: 0.25.0 апгрейднув rustls, через що дефолтний crypto-provider став `aws-lc-rs`
(біль крос-компіляції). 0.25.1 (21.11.2025) додав флаг `use-rustls-no-provider`, який дозволяє
обирати backend і уникнути aws-lc — саме це просив коментатор. Поверхня rumqttc у проєкті мала
(`MqttOptions::new` + сетери, `AsyncClient`, `QoS`, `Packet`, `Outgoing`, `ConnectionError`
обгорнутий opaque), тож міграція дешева.
- [x] Bump `rumqttc 0.24 → 0.25.1` у всіх місцях: `Cargo.toml` (deps + dev-deps),
  `core/Cargo.toml`, опційна залежність у `mqtt-topic-engine/Cargo.toml`.
- [x] Додати passthrough feature-флаг `rumqttc-use-rustls-no-provider`.
- [x] Приклад `004_hello_world_tls.rs` — компілюється без змін (rustls `ClientConfig` шлях сумісний).
- [x] Обробка помилок (`StateError::ConnectionAborted`) — безпечно, `error.rs` обгортає
  `ConnectionError` opaque; підтверджено збіркою.
- [x] Оновлено `Cargo.lock`; матриця бекендів збирається:
  rustls(+aws-lc, default) / rustls-no-provider / native-tls / no-TLS / websocket.
- [x] Прогнати тести з піднятим брокером: 10/10 інтеграційних тестів проти живого EMQX
  (real connect→publish→subscribe→deserialize для всіх серіалізаторів) — пройшли на 0.25.1.
- РІШЕННЯ по default: лишаємо `rumqttc-use-rustls` (з aws-lc) у default — працює out-of-the-box.
  `rumqttc-use-rustls-no-provider` — opt-in для крос-компіляції на 32-біт (треба свій provider, напр. ring).
  Винесення rumqttc у `[workspace.dependencies]` — лишилось у низькому пріоритеті.

### Тести
- [x] **Офлайн codegen-тест на `serializer = Type`** — `test_custom_serializer_generation`
  у `macros/src/codegen_test.rs`: перевіряє `clone_with_serializer`, тип `JsonSerializer` у
  сигнатурах, наявність методів, і `NotPresent("TestStructClient")`/`NotPresent("TestStructExt")`
  (TypedClient вимкнено). Маркери звірені з `naming.rs`. (trybuild — лишився опційним.)
- [x] **Інтеграційний тест per-topic serializer через живий брокер** —
  `tests/serializer_macro_integration.rs`: `#[mqtt_topic(..., serializer = JsonSerializer)]`,
  round-trip publish→subscribe→deserialize з **assert** цілісності даних; тихо деградує без брокера.
  Пройшов проти EMQX.
- [ ] **Хоч мінімальні unit-тести для `core`** (зараз 0 у lib): конструювання топіків/конфігу,
  парсинг URL — без брокера.

> ⚠️ **Документація — у самому кінці** (після стабілізації API/флагів), щоб не переписувати двічі.
> Див. блок "📚 Документація (фінал)" нижче.

---

## 🟠 Середній пріоритет — якість/UX

### Код
- [x] **Прибрати дублювання в `macros/src/codegen.rs`** — три методи
  (`generate_builder/subscriber/publisher_methods`) тепер обчислюють `(serializer_ty, client_expr,
  where_clause/extra_bounds)` один раз, а тіло `quote!` єдине. `default_pattern()` винесено з гілок.
  Підтверджено 37 тестами macros (включно з новим serializer-тестом) + build example 102. Без нових clippy-warning.
- [x] **Re-export `Transport` і TLS-типів** — `Transport` з core (+ prelude); `tokio_rustls`/`rustls`
  з root під rustls-feature (version-matched). Приклад `004` більше не імпортує `rumqttc::*`;
  додано `[[example]]` required-features. (коміт 995680b)

### Реліз-гігієна
- [x] **Виправити clippy/build-попередження** (4 шт.) — elided lifetimes в engine, unused import у
  core, `#[allow(clippy::assertions_on_constants)]` на feature-тестах. clippy `--workspace
  --all-targets` чисто. (коміт 995680b)
- [x] **`cargo package --list`** для всіх 5 крейтів (Сесія A). Знахідки + фікси:
  - LICENSE-файлів НЕ було в `core`/`macros`/`doc-macros` (лише root та engine). Скопійовано
    `LICENSE-MIT`+`LICENSE-APACHE` у всі три → тепер у пакеті (`cargo package --list` підтверджує;
    untracked → довелося `git add` вручну, hook їх не підхоплює).
  - **Пакувальна діра root**: root сидить у корені репо → `cargo package` тягнув **84 файли**, серед
    них `.github/dev-docs/PLAN-*`+`TODO-*` (внутрішні доки), CI-конфіги, `dev/certs/key.pem`
    (приватний ключ!), `examples/internal/` (20+ чернеток), `check_result.txt`.
    (`Cargo.toml.orig`/`.cargo_vcs_info.json` у `--list` генерує сам cargo — це не репо-junk.)
    Додано `exclude=[...]` у root `Cargo.toml` (`.github/`, `.cargo/`, `dev/`, `docs/`, `scripts/`,
    `examples/internal/`, дрібний junk) → **84 → 33** легітимних файли. `examples/.env` лишено
    (навмисні безпечні дефолти, localhost, без секретів). Члени-крейти чисті, exclude не треба.
  - `README.md` входить у всі; `cargo package --list` потребує `--allow-dirty` на брудному дереві.
- [x] **CI `actions/cache@v3` → `@v4`** (4 входження в `ci.yml`). (Сесія A)
- [x] **Мінімальні core unit-тести** (`core/src/message_serializer.rs`, `#[cfg(test)] mod tests`):
  round-trip (serialize→deserialize→`assert_eq`) для кожного серіалізатора під своїм `#[cfg(feature)]`
  (bincode + serde-родина: json/messagepack/cbor/postcard/ron/flexbuffers). 7 тестів зелені.
  **Виняток**: `protobuf` НЕ покрито (потребує `prost::Message`/згенерований тип — поза «мінімально»;
  кандидат на пост-0.2.0). Принагідно cfg-гейтнуто `use serde::...` (передіснуючий
  unused-import warning під `--no-default-features`). (Сесія A)
- [x] **CI: живі тести реально виконуються, а не деградують.** CI вже піднімав EMQX, але тести
  тихо деградували (false-green). Додано env-гейт `MQTT_REQUIRE_BROKER` (panic у Err-гілці, лише в
  job `integration-test`; локально — м'яка деградація) + читання `MQTT_BROKER_URL`. У ci.yml: wait-loop
  на TCP 1883 (бо `emqx ping` ≠ listener готовий), enforcement + обидва інтеграційні тести
  (`serializers_integration` + `serializer_macro_integration`), прибрано мертвий `integration_tls`;
  job `test` більше не дублює інтеграційні тести (`--lib`/`--doc`) і покриває engine без paho;
  examples job читає `MQTT_BROKER` (не `MQTT_BROKER_URL`). Перевірено локально: гейт падає на мертвому
  брокері, проходить на живому; усі test-job команди зелені.
  - ТЕХ-БОРГ (не блокер): (1) `examples` job маскує помилки `|| echo` — впалий приклад дасть
    false-green навіть із брокером (треба enforcement-гейт як у тестах); (2) feature `paho-mqtt`
    engine не має compile-coverage у CI (свідомо — нативний C-лінк відсутній).
  - [ ] REVISIT: `.cargo/audit.toml` ігнорує 4 rustls-webpki адвайзорі (RUSTSEC-2026-0049/0098/0099/0104),
    бо фікс заблокований upstream (rumqttc 0.25.1 пінить старий rustls; новішого rumqttc немає).
    Прибрати ці ignore, щойно rumqttc оновить rustls/rustls-webpki на пропатчену гілку.

---

## 🟢 Низький пріоритет / опційно

- [ ] `compile_error!` при одночасно ввімкнених `rumqttc-use-native-tls` + `rumqttc-use-rustls`
  (зараз взаємовиключність не enforced). РІШЕННЯ: відкладено — спершу треба емпірично підтвердити,
  що це реальний конфлікт (rumqttc може трактувати TLS-бекенди адитивно), інакше guard заборонить
  валідну комбінацію.
- [x] Generic-серіалізатор у парсері — варіант **(А) зроблено** (Сесія A): error-меседж тепер пояснює,
  що приймається лише простий шлях-тип, і підказує обхід через type alias
  (`type MySer = MySerializer<Foo>;`). Варіант (Б) — реальна підтримка `Expr::Path`→`syn::Type` —
  відкладено (низька цінність, обхід через alias безболісний).
- [x] **mqtt-topic-engine standalone-готовність** (✅ РІШЕННЯ: публікуємо окремо, У СКОУПІ 0.2.0):
  - [x] Документувати публічні елементи + додано крейт-рівневий `#![warn(missing_docs)]` у lib.rs
    (без нього зняття локальних `#![allow(missing_docs)]` було б no-op — лінт за дефолтом `allow`).
    Знято allow у `topic_router`/`topic_matcher`/`topic_match` (скоуп розширено на `topic_match` —
    центральні `TopicMatch`/`TopicPath`); `clippy::missing_docs_in_private_items` лишився.
    `cargo doc --all-features` та `clippy --all-targets` — 0 warning.
  - [x] LICENSE: у README посилання `../LICENSE-*` → `LICENSE-*` (3 шт.); власні копії вже були.
  - [x] `get_max_qos_for_topic` — РІШЕННЯ: **лишаємо як заготовку під QoS-downgrade** (private +
    `#[allow(dead_code)]`, не впливає на docs.rs/clippy). Підсилено перехресні коментарі
    `get_max_qos_for_topic` ↔ TODO в `unsubscribe`.
  - [x] `CHANGELOG.md` для engine (стартує з 0.1.0).
  - [x] README engine підключено як крейт-док через `include_md_transformed!`; усі ```rust-блоки
    розмічено як doctests (no_run/run/ignore), приклади приведено до реального API (`try_match`
    бере `Arc<TopicPath>`!), додано 2 приклади (QoS-агрегація, resubscribe-on-reconnect).
    21 doctest зелені. Залежність `doc-macros` (non-optional, path+version) — за рішенням варіант A.
  - [x] ТЕХ-БОРГ docs.rs **зроблено** (Сесія A): (1) усі README-лінки LICENSE → абсолютні
    GitHub-URL (`.../blob/main/LICENSE-*`) у root/core/macros/engine (doc-macros — лінка не було);
    (2) `[package.metadata.docs.rs]`: engine = `features=["rumqttc","ntex-mqtt"]` (НЕ all-features —
    paho лінкує нативну C-lib і завалив би docs.rs; обидві обрані фічі — pure-Rust; емпірично
    зібрано разом); root та core = `all-features=true` (вони не залежать від paho).
- [x] Винести `rumqttc` у `[workspace.dependencies]` — члени на `workspace = true`. (коміт 995680b)

---

## 📚 Документація (фінал — робиться ОСТАННЬОЮ, після стабілізації API/флагів)
- [ ] **README: секція "Per-topic serializer override"** — синтаксис `#[mqtt_topic("...", serializer = JsonSerializer)]`,
  коли вживати, обмеження (вимикається TypedClient; підтримуються лише не-generic типи).
- [ ] **README: секція "TLS / transport features"** — це прямо те, що просив коментатор.
  Матриця: `rumqttc-use-rustls` (+ aws-lc) / `rumqttc-use-rustls-no-provider` (свій provider, напр. ring) /
  `rumqttc-use-native-tls` / no-TLS (default-features = false) / `rumqttc-websocket` / `rumqttc-proxy`.
  Як вимкнути TLS повністю, як уникнути aws-lc на 32-біт через no-provider. Приклад вибору backend.
- [ ] **Оновити версію в усіх доках** `0.1.0 → 0.2.0` (README install-snippet, COMPARISON, examples/README).
- [ ] Фіналізувати CHANGELOG (дата релізу замість TBD).

---

## ✅ Фінальний чеклист перед `cargo publish`
- [ ] `cargo build --workspace --all-targets` без warning.
- [ ] `cargo test --workspace` зелене (з піднятим брокером — і живі тести).
- [ ] `cargo clippy --workspace --all-targets` чисто.
- [ ] CHANGELOG: підставити дату релізу замість `TBD`.
- [ ] Узгодженість версій (root/core/macros = 0.2.0; engine/doc-macros = 0.1.0).
- [ ] `cargo publish --dry-run` для кожного крейта в правильному порядку
  (engine/doc-macros → core → macros → root).
- [ ] Git tag `v0.2.0`.

---

## Прийняті рішення
- **rumqttc 0.24 → 0.25.1**: ✅ мігруємо у складі 0.2.0. Причина: 0.25.1 додав
  `use-rustls-no-provider`, що прямо закриває скаргу про aws-lc; поверхня rumqttc у проєкті мала,
  тож міграція дешева; лишатись на 0.24 = обходити проблему, а не розв'язувати. Деталі — у
  високопріоритетному блоці "Міграція rumqttc" вище.
- **Без власної TLS-абстракції**: коментатор просив гнучкі флаги, не обгортку. Достатньо
  passthrough-флагів + re-export `Transport`/TLS-типів для зручності.
- **Публікація крейтів**: `mqtt-topic-engine` — публікуємо окремо (standalone-цінність для спільноти).
  `doc-macros` — **лишаємо назавжди** (варіант A): авто-конвертація GitHub→docs.rs посилань потрібна,
  тож зайвий internal-крейт у реєстрі — прийнятна ціна. Варіант C (викинути) відхилено.
- **Версіонування — НЕЗАЛЕЖНЕ (не lockstep)**: клієнтські крейти (root/core/macros) = 0.2.0;
  standalone `mqtt-topic-engine` і internal `doc-macros` = 0.1.0 (їхній перший реліз; версія
  відображає власний API крейта, а не клієнта).
