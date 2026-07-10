# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This crate provides the procedural macros for
[`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client). For the full,
user-facing changelog see the
[workspace CHANGELOG](https://github.com/holovskyi/mqtt-typed-client/blob/main/CHANGELOG.md).

## [0.3.0] - 2026-07-10

### Added
- `MessageMeta` support in `#[mqtt_topic]`: structs may declare optional `meta`
  and `topic` fields; codegen accepts either the bare type (`MessageMeta` /
  `TopicMatch`, handed to you owned) or the shared `Arc<...>` (zero-copy).

### Changed
- `payload` / `topic` / `meta` are now reserved field names: using one as a
  `{...}` wildcard while also declaring the same-named field is a compile error
  (a `{meta}` wildcard with no `meta` field still compiles as a plain parameter).
  Generated `from_mqtt_message` impls now thread the `meta: Arc<MessageMeta>`
  argument automatically.

## [0.2.0] - 2026-06-27

### Added
- Per-topic serializer override via the `mqtt_topic` attribute:
  `#[mqtt_topic("...", serializer = JsonSerializer)]`. The error message now
  guides generic serializers toward a type alias.

### Changed
- Internal: deduplicated the serializer code generation paths (no public API change).

## [0.1.0] - 2025-07-27

- Initial release (published as part of `mqtt-typed-client` 0.1.0).
