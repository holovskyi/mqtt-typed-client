# План: реліз 0.2.0 з backward-compat shim для осмислених типів

## Контекст
Після виділення topic-движка в окремий крейт `mqtt-topic-engine` публічний API `core` змінився.
Опубліковано лише 0.1.0 (crates.io). Обрано **стратегію 2**: випустити `0.2.0` (свідомий breaking
у межах 0.x), але додати compat-shim для типів, якими реально міг користуватись клієнт, щоб без
потреби не ламати ранніх користувачів. Внутрішню механіку матчера навмисно НЕ відновлюємо.

## Поточний стан (факти)
- Пласкі шляхи `topic::Type` вже збережені (навіть надлишково) через реекспорт з engine.
- Compat-аліаси вже є тільки для `topic::topic_match` і `topic::topic_pattern_path`.
- Зникли підмодульні шляхи з v0.1.0: `topic::error`, `topic::topic_matcher`, `topic::topic_router`,
  `topic::topic_pattern_item`.
- Три типи не реекспортовані ніде в core: `TopicMatcherNode<T>`, `Len` (trait), `TopicMatchError` (enum).
- core залежить від engine з default features (`router`, `lru-cache` увімкнені) + `rumqttc`.
- Усі типи живі в engine і досяжні.
- Видалені 4 підмодульні шляхи (`error/topic_matcher/topic_router/topic_pattern_item`) внутрішніх
  вжитків НЕ мають. АЛЕ збережені shim-шляхи (`topic_match`, `topic_pattern_path`) активно
  використовуються в core/macros/examples і їх чіпати НЕ можна. Зокрема `macros/src/codegen.rs:134`
  генерує у користувацький код `::mqtt_typed_client_core::topic::topic_match::TopicMatch` — це
  інваріант контракту macro-виводу.
- `topic::topic_match::TopicMatchError` теж був публічним у v0.1.0 і зараз зламаний (наявний shim
  реекспортує лише `{TopicMatch, TopicPath}`).
- Усі крейти версії 0.1.0. CHANGELOG має секцію `[Unreleased]`.

## Рішення по типах

### Shim (відновити шлях у core, бо це осмислений користувацький API):
- `topic::error::*` → `MatcherResult, PatternResult, RouterResult, TopicError, TopicResult, limits, validation`
- `topic::topic_router::{SubscriptionId, TopicRouter, TopicRouterError}`
- `topic::topic_pattern_item::{TopicPatternError, TopicPatternItem}`
- `topic::topic_matcher::TopicMatcherError`
- пласко `topic::TopicMatchError` (зараз відсутній)

### НЕ відновлювати (документований breaking, внутрішня механіка):
- `TopicMatcherNode<T>` — вузол дерева матчера, був pub випадково.
- `Len` — внутрішній трейт.

## Кроки

### Крок 1. Розширити `core/src/topic.rs` shim-реекспортами
1. Додати у плаский блок тип `TopicMatchError`
   (через `pub use mqtt_topic_engine::topic_match::TopicMatchError;` — бо engine не реекспортує
   його з кореня, лише з підмодуля `topic_match`).
2a. ДОПОВНИТИ наявний `pub mod topic_match` (він уже існує!) типом `TopicMatchError`:
   `pub use mqtt_topic_engine::topic_match::{TopicMatch, TopicMatchError, TopicPath};`
2b. СТВОРИТИ нові підмодульні compat-аліаси (дзеркало v0.1.0), кожен з doc-comment "Re-exported for
   backward compatibility":
   - `pub mod error { pub use mqtt_topic_engine::error::{MatcherResult, PatternResult, RouterResult, TopicError, TopicResult, limits, validation}; }`
   - `pub mod topic_pattern_item { pub use mqtt_topic_engine::topic_pattern_item::{TopicPatternError, TopicPatternItem}; }`
   - `pub mod topic_matcher { pub use mqtt_topic_engine::topic_matcher::TopicMatcherError; }`  (БЕЗ TopicMatcherNode/Len)
   - `pub mod topic_router { pub use mqtt_topic_engine::topic_router::{SubscriptionId, TopicRouter, TopicRouterError}; }`
3. Підмодулі `topic_matcher`/`topic_router` в engine за feature `router`. core вмикає його через
   default engine, тож працює. Для надійності обгорнути ці два аліаси у
   `#[cfg(feature = "...")]`? — РІШЕННЯ: НЕ обгортати, бо core безумовно тягне router (default features
   engine не вимкнені). Якщо в майбутньому core почне вимикати — повернутись до цього.

### Крок 2. Bump версій до 0.2.0
- Підняти `version = "0.2.0"`: root `Cargo.toml`, `core/Cargo.toml`, `macros/Cargo.toml`.
- Оновити version-вимоги path-залежностей (інакше workspace НЕ збереться — `^0.1.0` не задовольняє 0.2.0):
  - `root → core` (`Cargo.toml:19`) 0.1.0→0.2.0
  - `root → macros` (`Cargo.toml:21`) 0.1.0→0.2.0
  - **`macros → core` (`macros/Cargo.toml:21` deps І `:25` dev-deps)** 0.1.0→0.2.0 — БЛОКЕР, легко пропустити.
- НЕ чіпати: `root → doc-macros` (engine/doc-macros лишаються 0.1.0).
- `mqtt-topic-engine` і `doc-macros`: лишити 0.1.0. Вимога `core → engine` = `"0.1.0"` валідна, не чіпати.

### Крок 3. CHANGELOG.md
- Перейменувати `[Unreleased]` → `## [0.2.0] - <дата релізу>` (дату підставити при релізі).
- Секції:
  - `### Added`: per-topic serializer (`serializer = Type`), `clone_with_serializer`/`clone_with_custom_serializer`,
    нові TLS feature-флаги (`rumqttc-use-rustls`, `rumqttc-use-native-tls`, `rumqttc-url`, `rumqttc-websocket`,
    `rumqttc-proxy`), `CacheStrategy::capacity()`, нові крейти `mqtt-topic-engine` + `doc-macros`.
  - `### Changed`: topic-движок виділено в окремий крейт; default features тепер містять
    `rumqttc-url`, `rumqttc-use-rustls`; build.rs замінено proc-макросом `doc-macros`.
  - `### Removed (BREAKING)`: публічні типи `TopicMatcherNode<T>`, `Len` (внутрішня механіка матчера).
  - `### Migration`: підмодульні шляхи (`topic::topic_router::*` тощо) збережені через compat-реекспорти;
    рекомендований шлях — кореневий `mqtt_typed_client_core::{Type}`.
- Оновити посилання порівняння внизу: додати `[0.2.0]` compare-лінк.

### Крок 4. Верифікація
- `cargo build --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets` (без нових попереджень)
- Compile-check, що відновлені шляхи реально резолвляться: тимчасовий doctest/тест з
  `use mqtt_typed_client_core::topic::topic_router::TopicRouter;` тощо (або перевірити вручну,
  потім прибрати).
- `cargo package --list -p mqtt-typed-client` та `-p mqtt-topic-engine` — переконатись, що
  README та потрібні файли пакуються (не блокер цього плану, але суміжне).

## Поза обсягом цього плану (окремо)
- Повернення дефолтної TLS-фічі (для 0.2.0 свідомо лишаємо нову default).
- Решта якісних доробок (тести core, документація router/matcher, дублювання codegen).
- Власне публікація на crates.io.

## Ризики
- Якщо хтось у engine все ж не вмикає `router`, аліаси `topic_router`/`topic_matcher` не зкомпілюються —
  пом'якшення: core гарантує router через default features engine.
- Версійний рассинхрон: engine лишається 0.1.0, а core/root 0.2.0 — переконатись, що Cargo
  version-вимоги на engine коректні (`version = "0.1.0"`).
