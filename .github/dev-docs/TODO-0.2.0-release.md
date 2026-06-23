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
- [ ] **`cargo package --list`** для root, core, macros, engine — переконатися що `README.md`,
  `examples/README.md` (потрібен для `include_md_transformed!`), LICENSE-файли потрапляють у пакет.
  Інакше docs.rs впаде.
- [ ] **CI: підняти EMQX-брокер** під час інтеграційних тестів (`dev/docker-compose.yml`), щоб
  живі тести реально виконувались, а не деградували.

---

## 🟢 Низький пріоритет / опційно

- [ ] `compile_error!` при одночасно ввімкнених `rumqttc-use-native-tls` + `rumqttc-use-rustls`
  (зараз взаємовиключність не enforced).
- [ ] Підтримати generic-серіалізатор `MySerializer<Foo>` у парсері (`macros/src/lib.rs` — зараз
  лише `Expr::Path`) АБО уточнити обмеження в error-меседжі.
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
  - [ ] ТЕХ-БОРГ docs.rs (НЕ блокер релізу, на потім): (1) відносні лінки `[LICENSE-MIT](LICENSE-MIT)`
    у README → 404 на docs.rs (зробити абсолютними або прибрати); (2) `[package.metadata.docs.rs]`:
    НЕ ставити `all-features=true` (увімкне paho → docs.rs-білд шукатиме нативну C-lib і впаде);
    радше `features=["rumqttc"]` (+ntex, обидва pure-Rust), щоб показати QoS-конверсії без нативу.
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
