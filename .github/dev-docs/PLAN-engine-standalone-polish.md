# PLAN: mqtt-topic-engine standalone-поліш (Сесія 2)

Мета: довести `mqtt-topic-engine` до якості, придатної для **окремої публікації** на crates.io
(він у скоупі 0.2.0, версія власна = 0.1.0). Фокус — docs.rs-готовність і коректні метадані пакета,
без зміни поведінки.

Контекст: крейт уже має `LICENSE-APACHE`/`LICENSE-MIT` копії у власному каталозі, коректний
`[package]` (license, repository, readme, keywords, categories). `cargo clippy --workspace` чисто.

---

## Зміни

### 1. README: виправити LICENSE-посилання (3 шт.)
Файл `mqtt-topic-engine/README.md`. Зараз посилаються на батьківський каталог (`../LICENSE-*`),
що НЕ рендериться на crates.io (пакет містить лише власний каталог).
- Рядок 9: badge-лінк `](../LICENSE-MIT)` → `](LICENSE-MIT)`.
- Рядок 660: `[LICENSE-APACHE](../LICENSE-APACHE)` → `[LICENSE-APACHE](LICENSE-APACHE)`.
- Рядок 661: `[LICENSE-MIT](../LICENSE-MIT)` → `[LICENSE-MIT](LICENSE-MIT)`.
Локальні копії вже існують, тож відносні лінки `LICENSE-MIT`/`LICENSE-APACHE` будуть валідні в пакеті.

### 2. Зняти `#![allow(missing_docs)]` + документувати публічні елементи
Цілі (узгоджено з користувачем): **`topic_router.rs`** та **`topic_matcher.rs`**.
(Поза скоупом цього проходу: `topic_match.rs` — теж має allow, але це окремий блок; див. «Відкриті питання».)

**`topic_router.rs`** — прибрати рядок 2 `#![allow(missing_docs)]` (рядок 1
`clippy::missing_docs_in_private_items` ЛИШАЄТЬСЯ — приватні поля/типи не документуємо).
Додати `///`-доки публічним елементам, де їх немає:
- `pub struct TopicRouter<T>` (рядок 80) — призначення: routing-таблиця підписок із
  matching за MQTT-патернами; параметр `T` — дані підписника.
- `pub fn new` (93), `add_subscription` (101), `unsubscribe` (136), `get_subscribers` (159),
  `get_active_subscriptions` (177). Решта (`get_topics_for_unsubscribe`, `get_topics_for_resubscribe`,
  `cleanup`, `get_topic_by_id`) уже мають доки — не чіпати.
  Для `add_subscription` документувати семантику повернення `(bool needs_subscribe, SubscriptionId)`.
  Для `unsubscribe` — `(bool topic_now_empty, TopicPatternPath)`.

**`topic_matcher.rs`** — прибрати рядок 2 `#![allow(missing_docs)]` (рядок 1 ЛИШАЄТЬСЯ).
- `pub struct TopicMatcherNode<T>` (51) — уже має doc-comment, ОК.
- `pub trait Len` (65) — додати doc на трейт + на методи `len`/`is_empty` (публічні елементи трейта).
- `pub fn new` (98) — уже має doc, ОК.
- `pub fn is_empty` (107) — додати doc.
- `get_or_create_subscription_table` (117), `update_node` (151), `find_by_path` (257) — уже мають доки.
- `collect_active_subscriptions` (306) під `#[cfg(test)]` — missing_docs не діє в test-only? Дія:
  лишити як є (test-only елементи). Якщо лінт спрацює — додати короткий doc.

Перевірка повноти: `cargo doc -p mqtt-topic-engine --all-features --no-deps` має зібратися
**без warning** (missing_docs стане deny-able). Прогнати також `--no-default-features` та з
`--features router` окремо (бо `topic_router`/`topic_matcher` за feature `router`).

### 3. Підсилити зв'язок «мертвої» `get_max_qos_for_topic` ↔ TODO (РІШЕННЯ: лишаємо)
Файл `topic_router.rs`. Функцію НЕ видаляємо (private + `#[allow(dead_code)]`, не впливає на
публічний API/docs.rs; це заготовка під майбутній QoS-downgrade). Дія — лише підсилити перехресне
посилання у коментарях:
- У doc-коментарі `get_max_qos_for_topic` (рядки 183-185) — явно вказати, що це заготовка під
  QoS-downgrade у `unsubscribe`, і що зараз max-QoS-логіка інлайниться в `add_subscription`.
- У TODO-коментарі `unsubscribe` (рядки 140-142) — додати посилання на `get_max_qos_for_topic`
  як готовий helper для майбутньої реалізації.
Це нульовий ризик (лише коментарі).

### 4. `CHANGELOG.md` для engine (новий файл)
`mqtt-topic-engine/CHANGELOG.md`, формат Keep a Changelog, стартова версія `[0.1.0]`.
Оскільки це ПЕРШИЙ окремий реліз крейта — секція описує первинний публічний функціонал:
topic pattern matching, `TopicRouter` (feature `router`), LRU-кеш (feature `lru-cache`),
QoS-типи для rumqttc/paho-mqtt/ntex-mqtt (відповідні features), `CacheStrategy`.
Дата — TBD (підставимо разом із релізом, як і в кореневому CHANGELOG).

---

## Поза скоупом (не робимо в цьому проході)
- Звуження видимості `TopicMatcherNode`/`Len` (вони `pub` у `pub mod topic_matcher`). Для першого
  релізу лишаємо публічними + документованими; рефакторинг видимості — окремо, якщо взагалі.
- `topic_match.rs` `#![allow(missing_docs)]` — окремий блок (експортує центральні `TopicMatch`/`TopicPath`).

## Відкриті питання для критика
1. Чи треба в ЦЬОМУ проході також зняти `#![allow(missing_docs)]` з `topic_match.rs`
   (для цілісної docs.rs-картини), чи лишити поза скоупом як зазначено?
2. Чи коректно лишати `clippy::missing_docs_in_private_items` allow — чи для standalone-крейта
   варто документувати й приватні елементи?
3. Чи `#[cfg(test)] pub fn collect_active_subscriptions` потребує doc після зняття allow
   (missing_docs на test-only pub items)?
4. Чи нічого не зламає зняття allow у feature-матриці (no-default-features не має `router`,
   тож `topic_router`/`topic_matcher` не компілюються — allow там нерелевантний)?

## Перевірка (після імплементації)
- `cargo doc -p mqtt-topic-engine --all-features --no-deps` — без warning.
- `cargo build -p mqtt-topic-engine --no-default-features` — ОК.
- `cargo clippy -p mqtt-topic-engine --all-features --all-targets` — чисто.
- `cargo test -p mqtt-topic-engine --all-features` — зелене.
- `cargo package -p mqtt-topic-engine --list --allow-dirty` — README + обидва LICENSE у пакеті.

## Коміт
Один коміт (через hook, що git-add'ить усе). Повідомлення — секція про engine standalone-готовність.
Co-Authored-By trailer.
