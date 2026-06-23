# План: re-export Transport + TLS-типів (щоб не лізти в rumqttc напряму)

Мета: користувач (і приклад `004_hello_world_tls.rs`) не повинен додавати `rumqttc` прямою
залежністю чи знати шлях `rumqttc::tokio_rustls::rustls::{...}`. НЕ робимо абстракцію — лише
зручні re-export, version-matched із тим rustls, що тягне rumqttc.

## Факти (звірено з кодом)
- Приклад `004` тягне: `rumqttc::Transport`, `rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore}`,
  викликає `Transport::tls_with_config(...)` і `config.set_transport(...)`.
- core вже re-export'ить `rumqttc::MqttOptions` (`core/src/lib.rs:153`) і `rumqttc::QoS` (:155).
- core: `rumqttc = { workspace = true, features = ["url"] }` — БЕЗ TLS. `tokio_rustls` у core
  недосяжний без unified use-rustls, а core не має власної TLS-feature → cfg в core неможливий без
  перебудови features.
- **root** (`mqtt-typed-client`): має пряму залежність `rumqttc = { workspace = true }` І визначає
  features `rumqttc-use-rustls`, `rumqttc-use-rustls-no-provider`, `rumqttc-use-native-tls` тощо.
  → root МОЖЕ cfg-gейтити re-export на власні features без змін у core.
- `Transport` — частина базового API rumqttc, доступна завжди (не TLS-gated).

## Дизайн (мінімальний, без перебудови core-features)

### 1. `Transport` — re-export з core, безумовно
У `core/src/lib.rs` поряд із MqttOptions/QoS:
```rust
pub use rumqttc::Transport;
```
Доступний завжди; через `pub use mqtt_typed_client_core::*;` потрапляє і в root.
(Альтернатива — у root; але core логічніше, бо там уже MqttOptions/QoS.)

### 2. rustls-типи — re-export з ROOT, gated на TLS-features
tokio_rustls присутній при rustls-бекенді (звичайному АБО no-provider). У `src/lib.rs` (root):
```rust
/// Re-export of rumqttc's bundled rustls stack, so you can build a
/// `ClientConfig` without depending on `rumqttc`/`rustls` directly (and with a
/// version guaranteed to match the transport). Available with a rustls feature.
#[cfg(any(feature = "rumqttc-use-rustls", feature = "rumqttc-use-rustls-no-provider"))]
pub use rumqttc::tokio_rustls;
```
Користувач: `use mqtt_typed_client::tokio_rustls::rustls::{ClientConfig, RootCertStore};`.

(native-tls: приклад його не використовує; за бажанням окремим пунктом re-export
`#[cfg(feature="rumqttc-use-native-tls")] pub use rumqttc::tokio_native_tls;` — НЕ в цьому плані,
щоб не роздувати. Зафіксувати як опційне.)

### 3. Оновити приклад `004_hello_world_tls.rs`
- Прибрати `use rumqttc::Transport;` і `use rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore};`.
- Додати `use mqtt_typed_client::{Transport, tokio_rustls::rustls::{ClientConfig, RootCertStore}};`
  (або через повний шлях). `rumqttc` із прямих use прикладу зникає.
- Перевірити, чи приклад досі має `rumqttc` у dev-deps root для чогось іншого — якщо ні, він і так
  лишається транзитивно; головне, що КОРИСТУВАЦЬКИЙ код прикладу більше не імпортує rumqttc.

## Верифікація
1. `cargo build --example 004_hello_world_tls` (default features = rustls) — компілюється.
2. `cargo build -p mqtt-typed-client --no-default-features --features json` — re-export tokio_rustls
   НЕ присутній (cfg вимкнений), збірка ОК (Transport усе одно є).
3. `--no-default-features --features "json,rumqttc-url,rumqttc-use-rustls-no-provider"` — tokio_rustls
   re-export присутній, збирається.
4. `cargo build --workspace --all-targets` + `cargo clippy` чисто.
5. (опц.) запустити приклад 004 проти брокера на 8883 — поза скоупом (потребує TLS-listener/сертифікати).

## Ризики / відкриті питання
- Чи `Transport` справді доступний у rumqttc без TLS-features? (Очікую так — це базовий enum; але його
  TLS-варіанти конструюються лише з TLS. Сам тип має існувати завжди. ПЕРЕВІРИТИ збіркою no-TLS.)
- Чи `pub use rumqttc::tokio_rustls;` у root не конфліктує з glob `pub use mqtt_typed_client_core::*;`
  (core не re-export'ить tokio_rustls, тож конфлікту нема).
- Чи назва `tokio_rustls` достатньо зрозуміла користувачу, чи краще загорнути у власний модуль
  `pub mod tls { pub use rumqttc::tokio_rustls::rustls; }` для чистішого шляху
  `mqtt_typed_client::tls::rustls::...`? — рішення винести в критику.

## Поза скоупом
- native-tls re-export (окремо, опційно).
- Документація (README TLS-секція) — у фінальний блок.
- Повноцінна TLS-абстракція — свідомо НЕ робимо.
