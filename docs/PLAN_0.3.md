# 0.3 Release Plan

*Drafted 2026-07-08. Theme: **deepen the delivery guarantees** — metadata,
ack surfacing, connection observability — and redesign the public API so that
MQTT 5 (0.4) and the backend switch land later WITHOUT a second breaking
release. Everything here is v4/MQTT 3.1.1 functionally; every public type is
designed v5-first with v4 as the degenerate case.*

*Backing research: [FUTURE_WORK_RESEARCH.md](./FUTURE_WORK_RESEARCH.md) §2
(ack correlation), [research/RUMQTTC_NEXT_AUDIT_2026.md](./research/RUMQTTC_NEXT_AUDIT_2026.md)
(backend decision), [research/DUAL_PROTOCOL_API_DESIGN_2026.md](./research/DUAL_PROTOCOL_API_DESIGN_2026.md)
(API shape, locked-vs-deferred list).*

## Scope

### 1. De-leak the public API (prerequisite for everything)

Replace every leaked backend type with our own protocol-neutral facade:

- Own `MqttOptions`-equivalent config (v5-shaped: `session_expiry`-style with
  documented v4 mapping, NOT `clean_session: bool` verbatim), `Transport`,
  `LastWill`. `MqttClientConfig.connection` stops exposing `rumqttc::MqttOptions`.
- `SubscriptionConfig.qos` and all public QoS surfaces use the engine's
  protocol-neutral `QoS` (mqtt-topic-engine/src/qos.rs).
- Own error enums (all `#[non_exhaustive]`); kill the
  `rumqttc::ConnectReturnCode` leak in
  `ConnectionEstablishmentError::BrokerRejected` (core/src/client/error.rs).
- Own reason-code enum designed as the **v5 superset**; v4 return codes map in.
- `ProtocolVersion` (`#[non_exhaustive]`, `V4` default) in config +
  `?protocol=` URL grammar: parsed now, `5` → clean "MQTT 5 arrives in 0.4"
  error.
- Root re-exports of `rumqttc`/`tokio_rustls` reviewed: keep only what a TLS
  user genuinely needs, behind our naming.
- Negative decision (locked): NO protocol type parameter, NO split public
  v4/v5 modules — unified surface, protocol is a runtime value.

### 2. `MessageMeta` — incoming message metadata

```rust
pub struct MessageMeta {
    pub qos: QoS,
    pub retain: bool,
    pub dup: bool,
    pub v5: Option<Mqtt5Meta>,   // None on v4; ALWAYS present, never cfg-gated
}

#[non_exhaustive]
pub struct Mqtt5Meta { /* user_properties, content_type, correlation_data,
                          response_topic, message_expiry — defined now,
                          always None/empty in 0.3 */ }
```

- Delivery via the macro's existing magic-field convention: an optional
  `meta: MessageMeta` field in the topic struct, auto-populated like
  `topic: Arc<TopicMatch>`. Non-declaring users pay nothing.
- Resolve the magic-name vs topic-parameter collision (a pattern like
  `"a/{meta}/b"`): explicit compile error + attribute escape hatch. Audit how
  `topic`/`payload` collisions behave today (suspected silent bug).
- Stop discarding `p.retain/qos/dup` in async_client.rs.

### 3. Ack surfacing

- **SubAck (minimal, backend-independent):** stop dropping
  `SubAck.return_codes` — surface broker-side subscription rejection and QoS
  downgrade (log + typed event/error to the subscriber).
- **Full correlation (publish → PUBACK/PUBCOMP future, subscribe → granted
  QoS)** rides on the rumqttc-next backend (`publish_tracked`/`subscribe_tracked`).
  Public shape: `publish()` returns a future resolving to a v5-shaped result
  (reason code; `Success` on v4). Design the public API now; wire it when the
  backend lands (this release if the backend switch happens in 0.3, else 0.4).
- `PublishOptions` builder (`#[non_exhaustive]`, private fields):
  qos + retain now; v5 knobs (expiry, user properties, content type) arrive
  additively in 0.4. Replaces loose (qos, retain) args where public.

### 4. Connection state observability

- `watch::Receiver<ConnectionState>`: `Connected` / `Reconnecting { attempt }`
  / `Disconnected { reason }` (terminal).
- Event-loop death after `MAX_CONSECUTIVE_ERRORS` becomes an explicit
  terminal state — no more silent zombie client.
- (Stretch) `ReconnectPolicy` as a config value; watch channel is its
  prerequisite either way.

### 5. Backpressure: fix + document only (design doc, no implementation)

- Fix the slow-consumer **ordering bug** in
  core/src/routing/subscription_manager.rs (message B can overtake message A
  parked in a slow-send task).
- Make the hardcoded 500 / 100 / 2s knobs configurable via
  `SubscriptionConfig`; document the buffer→grace→drop policy; expose drop
  visibility (counter or event).
- Full manual-acks/Receive-Maximum design → separate doc, implementation 0.4+.

### 6. Backend switch to rumqttc-v4-next (decision: adopt-with-mitigations)

- Swap `rumqttc` → `rumqttc-v4-next` **pinned to an audited git rev / next
  audited release** (crates.io 0.33.2 lacks audited fixes — see audit doc).
  Migration recipe validated by the maintainer's port (S–M, ~1 day):
  builder construction, `Broker::tcp`, `PublishOptions`, `Bytes` topics
  (reject non-UTF-8, do NOT `from_utf8_lossy`), explicit-transport story for
  `mqtts://`/`wss://` URLs. MSRV → 1.89, edition 2024 implications.
- Timing gate: coordinate with "eagle" (mqtt-typed-client-next#1) first; his
  response may add a QoS-downgrade PR and a crates.io release. If adoption
  slips, items 1–5 still ship on upstream rumqttc (SubAck-minimal only; full
  correlation moves to 0.4).
- `mqtt-topic-engine`'s rumqttc-interop feature gains a `-next` variant so
  upstream-rumqttc users keep working.

### Out of scope for 0.3

MQTT 5 wire support (0.4); `BackendClient` enum + v5 event loop; `Mqtt5Meta`
population; typed RPC; shared subscriptions; AsyncAPI export; no_std;
compression; offline queue. Upstreaming eagle's QoS-downgrade-on-unsubscribe
feature is welcome any time (independent of all of the above).

## Order of work

1. De-leak API (§1) — unblocks everything, no external dependency.
2. Ordering-bug fix + backpressure knobs (§5) — self-contained.
3. MessageMeta (§2) + macro work.
4. Connection state (§4).
5. SubAck minimal (§3) on whatever backend is current.
6. Backend swap (§6) + tracked-notice publish/subscribe API — gated on the
   eagle coordination outcome; may slip to 0.4 without blocking the release.

## Open items (external)

- eagle's response to mqtt-typed-client-next#1 (coordination, QoS-downgrade
  PR, release cadence).
- LabOverWire/mqtt-lib#100 response (mqtt5 client/broker split — 0.4+ signal
  only).
- File the two upstream bytebeamio/rumqtt bug issues from our audit
  (`clean()` doesn't clear `state.collision`; subscribe pkid reuse) — cheap,
  community-useful, independent of our backend choice.
