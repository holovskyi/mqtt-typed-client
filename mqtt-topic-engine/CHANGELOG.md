# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-07-10

### Added
- `#[derive(Clone)]` on `TopicMatch` (cheap: one `Arc` bump + two inline-vec
  copies). Enables the by-value `topic`/`meta` codegen in `mqtt-typed-client`.

## [0.1.1] - 2026-07-01

### Documentation
- README now leads with a pointer to [`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client)
  as the recommended entry point (this crate is its routing core; use it standalone
  only when building your own MQTT layer). Docs-only release — no code changes.

## [0.1.0] - 2026-06-27

First standalone release of `mqtt-topic-engine`, the topic pattern matching and
routing engine extracted from [`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client).

### Added
- MQTT topic pattern parsing and validation: `TopicPatternPath`, `TopicPatternItem`
  (literals, `+` single-level and `#` multi-level wildcards), with named parameters.
- Topic matching: `TopicPath` / `TopicMatch` with positional and named parameter
  capture (`get_param`, `get_named_param`).
- `CacheStrategy` for tuning pattern-compilation caching, with an optional LRU
  cache behind the `lru-cache` feature.
- Subscription routing behind the `router` feature: `TopicRouter`, `SubscriptionId`,
  and the underlying `TopicMatcherNode` trie, tracking effective broker QoS.
- QoS type with conversions to/from `rumqttc`, `paho-mqtt`, and `ntex-mqtt` QoS
  types behind the respective `rumqttc` / `paho-mqtt` / `ntex-mqtt` features.
- `TopicPatternPath::try_match_str` — convenience wrapper that matches a topic
  given as a string (builds and shares the `TopicPath` for you); `try_match`
  remains for the hot path where one `Arc<TopicPath>` is reused across patterns.

### Documentation
- The crate-level docs now render the README, and the README's Rust examples are
  compiled as doctests (so they cannot silently drift from the API). Added
  worked examples for router QoS aggregation and resubscribe-after-reconnect.

### Features
- `default = ["router", "lru-cache"]`.
- `router` — subscription routing (`TopicRouter` and friends).
- `lru-cache` — LRU-backed pattern cache.
- `rumqttc` / `paho-mqtt` / `ntex-mqtt` — QoS interop with those client crates.
