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
- [ ] **Офлайн codegen-тест на `serializer = Type`**. Зараз 0 покриття (усі тести `custom_serializer: None`).
  - Додати в `macros/src/codegen_test.rs` кейс з `custom_serializer: Some(...)`; перевірити що
    згенерований код містить `clone_with_serializer::<...>()` і правильні where-bounds, і що
    TypedClient НЕ генерується.
  - Бажано `trybuild` compile-pass тест для `#[mqtt_topic("...", serializer = JsonSerializer)]`.
- [ ] **Інтеграційний тест per-topic serializer через живий брокер** (round-trip publish→subscribe
  з кастомним серіалізатором). Базувати на патерні `tests/serializers_integration.rs`, але
  тригерити через макрос. Має деградувати без брокера, як решта.
- [ ] **Хоч мінімальні unit-тести для `core`** (зараз 0 у lib): конструювання топіків/конфігу,
  парсинг URL — без брокера.

### Документація
- [ ] **README: секція "Per-topic serializer override"** — синтаксис `#[mqtt_topic("...", serializer = JsonSerializer)]`,
  коли вживати, обмеження (вимикається TypedClient; підтримуються лише не-generic типи).
- [ ] **README: секція "TLS / transport features"** — це прямо те, що просив коментатор.
  Матриця: `rumqttc-use-rustls` (+ aws-lc) / `rumqttc-use-rustls-no-provider` (свій provider, напр. ring) /
  `rumqttc-use-native-tls` / no-TLS (default-features = false) / `rumqttc-websocket` / `rumqttc-proxy`.
  Як вимкнути TLS повністю, як уникнути aws-lc на 32-біт через no-provider. Приклад вибору backend.
- [ ] **Оновити версію в усіх доках** `0.1.0 → 0.2.0` (README рядок ~148 install-snippet,
  COMPARISON, examples/README за потреби).

---

## 🟠 Середній пріоритет — якість/UX

### Код
- [ ] **Прибрати дублювання в `macros/src/codegen.rs`** (гілки with/without serializer у
  `generate_builder/subscriber/publisher_methods`, ~120 рядків). Обчислити токен серіалізатора +
  bounds + `clone`/`clone_with_serializer` один раз, шаблонізувати. **Тільки ПІСЛЯ** офлайн-тесту
  (страхувальна сітка для рефактора proc-макро).
- [ ] **Re-export `Transport` і потрібних TLS-типів** з кореня крейта, щоб приклад
  `004_hello_world_tls.rs` не ліз напряму в `rumqttc::tokio_rustls::rustls::{...}` і користувач
  не мусив додавати rumqttc прямою залежністю. (НЕ повноцінна абстракція — лише зручний re-export.)

### Реліз-гігієна
- [ ] **Виправити clippy/build-попередження** (4 шт.):
  - `mqtt-topic-engine/src/topic_pattern_item.rs:~295` та `topic_pattern_path.rs` — elided lifetime (`Iter<'_, ...>`).
  - `core/src/structured/subscriber.rs:6` — невикористаний `use tracing::error`.
  - `src/lib.rs:~277` — const-assertion у тестовому коді.
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
- [ ] **mqtt-topic-engine standalone-готовність** (публікується окремо):
  - [ ] Документувати pub-модулі `topic_router` / `topic_matcher` (зняти `#![allow(missing_docs)]`).
  - [ ] LICENSE: прибрати у README посилання `../LICENSE-MIT` (не рендериться на crates.io) — власні копії.
  - [ ] Прибрати/задіяти мертвий `get_max_qos_for_topic`; рішення по TODO QoS-downgrade (`topic_router.rs`).
  - [ ] `CHANGELOG.md` для engine.
- [ ] Рішення щодо публічності внутрішнього `doc-macros` (зараз публікується публічно).
- [ ] Винести `rumqttc = "0.24"` у `[workspace.dependencies]` (хардкод у 3 місцях).

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
