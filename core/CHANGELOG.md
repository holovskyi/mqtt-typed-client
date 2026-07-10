# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This is the core crate of [`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client).
For the full, user-facing changelog see the
[workspace CHANGELOG](https://github.com/holovskyi/mqtt-typed-client/blob/main/CHANGELOG.md).

## [0.3.0] - 2026-07-10

### Changed — public API de-leak (BREAKING)
- The public API no longer exposes `rumqttc` types (three documented exemptions:
  the semver-exempt `backend` module, the `rustls` re-export, and `QoS`'s optional
  rumqttc conversions). This includes own error types (`ConnectReasonCode`,
  `BackendError`, `ClientOperationError`, `UrlParseError`, all error enums
  `#[non_exhaustive]`), a `ConnectionOptions` facade with `SessionPolicy` /
  `ProtocolVersion` / own `Transport`/`TlsConfig` replacing `rumqttc::MqttOptions`,
  and protocol-neutral `QoS` in the public API. See the
  [workspace CHANGELOG](https://github.com/holovskyi/mqtt-typed-client/blob/main/CHANGELOG.md)
  for the full 0.2 → 0.3 migration table.

### Added
- Per-message metadata: `MessageMeta` (`qos`, `retain`, `dup`, plus a reserved
  `v5: Option<Mqtt5Meta>` slot, always `None` on MQTT 3.1.1) delivered with every
  message; `MessageMeta::new` is public.
- Configurable per-subscription backpressure: `SubscriptionConfig` gained
  `channel_capacity`, `slow_send_timeout`, and `max_parked_messages` (were
  hardcoded), with matching builder methods; `dropped_messages() -> u64` on the
  subscriber types reports messages dropped when the consumer cannot keep up.
- Connection state observability: `MqttClient::connection_state() ->
  watch::Receiver<ConnectionState>` reports the lifecycle via own
  `#[non_exhaustive]` protocol-neutral `ConnectionState` / `DisconnectReason` enums.
- `unstable-backend-api` feature: `ConnectionOptions::backend_tweak` for raw
  backend-options access (semver-exempt).

### Changed (BREAKING)
- Mid-layer `MqttSubscriber::receive()` now yields `ReceiveEvent` with named
  structs `IncomingMessage<T> { topic, meta, payload }` and
  `DecodeFailure<E> { topic, meta, error }` instead of tuples.
- `SubscriptionConfig` is now `#[non_exhaustive]`; `Subscriber::new` is now
  `pub(crate)`; `FromMqttMessage::from_mqtt_message` gained a
  `meta: Arc<MessageMeta>` argument.

### Fixed
- Slow-consumer message reordering: delivery is now strictly FIFO per subscriber
  (at most one in-flight send; buffer → grace → drop, drops never reorder).
- Zombie consumers on terminal event-loop death: subscriber channels are now
  closed on terminal death, so `receive()` yields `None` and consumer loops end.

## [0.2.0] - 2026-06-27

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
