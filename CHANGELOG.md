# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed — public API de-leak (BREAKING)

The public API no longer exposes `rumqttc` types anywhere. This is the
prerequisite for the planned backend switch and MQTT 5 support (0.4) landing
WITHOUT another breaking release. Migration map:

| 0.2 | 0.3 |
| --- | --- |
| `use rumqttc::QoS` / re-exported `QoS` | `mqtt_typed_client::QoS` (protocol-neutral, same variants) |
| `config.connection: rumqttc::MqttOptions` | `config.connection: ConnectionOptions` (own type) |
| `.set_keep_alive(d)` | `config.connection.keep_alive = d` |
| `.set_clean_session(true/false)` | `config.connection.session = SessionPolicy::CleanPerConnection / ::Resume` |
| `.set_credentials(u, p)` | `config.connection.credentials = Some(Credentials { .. })` |
| `.set_transport(Transport::tls_with_config(c))` | `config.connection.transport = Transport::Tls(c.into())` |
| `.set_inflight(..)`, `.set_max_packet_size(..)`, other backend knobs | `backend_tweak(..)` behind the `unstable-backend-api` feature (SEMVER-EXEMPT) |
| `from_url` → `rumqttc::OptionError` | `from_url` → `UrlParseError` (own type) |
| URL params `inflight_num`, `request_channel_capacity_num`, `max_request_batch_num`, `pending_throttle_usecs`, `max_*_packet_size_bytes`, `conn_timeout_secs` | rejected with an explicit error pointing to the escape hatch |
| `BrokerRejected { code: rumqttc::ConnectReturnCode }` | `BrokerRejected { code: ConnectReasonCode }` (v5-superset enum) |
| `MqttClientError::ClientOperation(rumqttc::ClientError)` | `ClientOperation(ClientOperationError)` |
| `Network(Box<rumqttc::ConnectionError>)` | `Network(BackendError)` (opaque, keeps Display + source chain) |
| feature `rumqttc-use-rustls` etc. | `tls-rustls`, `tls-rustls-no-provider`, `tls-native`, `websocket`, `proxy` (old names remain as deprecated aliases; removed in 0.4) |
| feature `rumqttc-url` | gone — URL parsing is built in |
| re-export `tokio_rustls` | gone (`rustls` is still re-exported under the TLS features) |

### Added
- `ConnectionOptions` with `SessionPolicy` (v5-shaped: `CleanPerConnection` /
  `Resume` / `ResumeFor` — the latter is MQTT 5 only and errors on v4),
  `ProtocolVersion` (`?protocol=4|5` in URLs; `V5` gives a clean "arrives in
  0.4" connect error), own `Transport`/`TlsConfig` enums.
- Connect-time validation instead of backend panics: sub-second `keep_alive`,
  `Resume` with an empty `client_id`, and unsupported transports now return
  `MqttClientError::ConfigurationValue`.
- `unstable-backend-api` feature: `ConnectionOptions::backend_tweak` gives raw
  access to backend options at connect time (semver-exempt), and
  `backend::rumqttc` re-exports the backend crate version-matched.
- All public error enums are `#[non_exhaustive]`; new `ConnectReasonCode`,
  `BackendError`, `ClientOperationError`, `UrlParseError` types.

### Changed
- crates.io `keywords`: replaced the redundant `tokio` with `typed` (identity
  term, less crowded) for `mqtt-typed-client` and `mqtt-typed-client-core`.
  Metadata-only; takes effect on the next published release.

## [0.2.0] - 2026-06-27

### Added
- Per-topic serializer override via the `mqtt_topic` macro attribute:
  `#[mqtt_topic("...", serializer = JsonSerializer)]`.
- `MqttClient::clone_with_serializer::<S>()` and `clone_with_custom_serializer(serializer)`.
- `CacheStrategy::capacity()` convenience method.
- Granular TLS / transport feature flags forwarding to `rumqttc`:
  `rumqttc-url`, `rumqttc-websocket`, `rumqttc-use-rustls`, `rumqttc-use-native-tls`,
  `rumqttc-proxy`. This lets you pick a TLS backend (rustls / native-tls) or build
  without TLS.
- `rumqttc-use-rustls-no-provider` feature — use rustls without bundling a crypto
  provider (e.g. `aws-lc-rs`), so you can bring your own (such as `ring`) and avoid
  the `aws-lc` cross-compilation pain on 32-bit / embedded targets.
- Re-exports so custom transports need no direct `rumqttc` dependency:
  `Transport` (always available, also in `prelude`), and — under a rustls feature —
  `tokio_rustls` and `rustls` (version-matched to the transport), for building a
  `ClientConfig` for `Transport::tls_with_config(...)`.
- New standalone crate `mqtt-topic-engine` — the topic pattern matching and routing
  engine, usable without the MQTT client.

### Changed
- **BREAKING (default features):** the default feature set now includes
  `rumqttc-url` and `rumqttc-use-rustls`, and `rumqttc` is pulled with
  `default-features = false`. If you relied on `rumqttc`'s default TLS being
  enabled implicitly, enable the corresponding `rumqttc-*` feature explicitly.
- The topic engine was extracted from `core` into the `mqtt-topic-engine` crate.
  Public types remain available through `mqtt_typed_client_core::topic::*`.
- Removed the `build.rs` documentation-generation step (supply-chain hardening —
  no build script). README and example docs are now embedded directly via
  `include_str!`, with example links pointing at absolute GitHub URLs so they
  resolve both on GitHub and on docs.rs.
- Upgraded `rumqttc` from 0.24 to 0.25.1. The public API surface used by this
  crate is unchanged; with default features (`rumqttc-use-rustls`) rustls now
  pulls `aws-lc-rs` as its crypto provider — use `rumqttc-use-rustls-no-provider`
  to opt out (see above).

### Removed
- **BREAKING:** the incidentally-public matcher internals `TopicMatcherNode<T>`
  and the `Len` trait are no longer part of the public API. They were never
  intended as a stable surface.

### Migration
- Recommended import path stays the curated root re-exports, e.g.
  `mqtt_typed_client_core::{CacheStrategy, TopicError, TopicPatternPath, ...}`.
- v0.1.0 submodule paths are preserved via backward-compat re-exports:
  `topic::error::*`, `topic::topic_router::*`, `topic::topic_pattern_item::*`,
  `topic::topic_matcher::TopicMatcherError`, `topic::topic_match::*`,
  `topic::topic_pattern_path::*`. `CacheStrategy` moved from
  `routing::subscription_manager` to `topic` (root re-export unchanged).
- `TopicMatchError` is now also available flat as `topic::TopicMatchError`.

## [0.1.0] - 2025-07-27

### Added
- Initial release of mqtt_typed_client
- Type-safe MQTT client with pattern-based routing
- Support for MQTT wildcard patterns (`+`, `#`)
- Automatic subscription management with graceful shutdown
- Pluggable serialization with BincodeSerializer included
- Comprehensive error handling with retry logic
- Production-ready async/await support built on tokio
- Memory-efficient implementation with proper backpressure handling

### Features
- `mqtt_topic` procedural macro for automatic code generation
- Structured subscribers with topic parameter extraction
- Last Will and Testament (LWT) message support
- Connection URL parsing and TLS support
- Subscription builder pattern for flexible configuration
- Typed client extensions for ergonomic API

[Unreleased]: https://github.com/holovskyi/mqtt-typed-client/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/holovskyi/mqtt-typed-client/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/holovskyi/mqtt-typed-client/releases/tag/v0.1.0
