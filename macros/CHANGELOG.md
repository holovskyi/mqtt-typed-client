# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This crate provides the procedural macros for
[`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client). For the full,
user-facing changelog see the
[workspace CHANGELOG](https://github.com/holovskyi/mqtt-typed-client/blob/main/CHANGELOG.md).

## [Unreleased]

## [0.2.0] - 2026-06-23

### Added
- Per-topic serializer override via the `mqtt_topic` attribute:
  `#[mqtt_topic("...", serializer = JsonSerializer)]`. The error message now
  guides generic serializers toward a type alias.

### Changed
- Internal: deduplicated the serializer code generation paths (no public API change).

## [0.1.0] - 2025-07-27

- Initial release (published as part of `mqtt-typed-client` 0.1.0).
