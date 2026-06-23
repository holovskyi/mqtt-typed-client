# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This is the core crate of [`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client).
For the full, user-facing changelog see the
[workspace CHANGELOG](https://github.com/holovskyi/mqtt-typed-client/blob/main/CHANGELOG.md).

## [Unreleased]

## [0.2.0] - 2026-06-23

### Added
- Multi-serializer support: `MqttClient::clone_with_serializer::<S>()` and
  `clone_with_custom_serializer(serializer)`.

### Changed
- The topic engine was extracted into the standalone
  [`mqtt-topic-engine`](https://crates.io/crates/mqtt-topic-engine) crate; its
  public types remain available through `mqtt_typed_client_core::topic::*`
  (v0.1.0 submodule paths preserved via re-exports).
- Upgraded `rumqttc` from 0.24 to 0.25.1.

### Removed
- **BREAKING:** the incidentally-public matcher internals `TopicMatcherNode<T>`
  and the `Len` trait are no longer part of the public API.

## [0.1.0] - 2025-07-27

- Initial release (published as part of `mqtt-typed-client` 0.1.0).
