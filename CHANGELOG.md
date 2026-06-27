# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
