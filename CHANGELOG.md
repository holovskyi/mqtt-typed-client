# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/holovskyi/mqtt-typed-client/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/holovskyi/mqtt-typed-client/releases/tag/v0.1.0
