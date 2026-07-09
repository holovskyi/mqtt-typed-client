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
- NOTE: the backpressure lag notification is NOT a `MessageMeta` field — it is a
  `ReceiveEvent::Lagged` variant on the `receive()` return (see §5 / step 2b),
  landed before this section. `MessageMeta` is only per-message protocol
  metadata.

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

### 5. Backpressure: ordering fix + knobs + drop notification

- **DONE (2026-07-09):** fixed the slow-consumer **ordering bug** in
  core/src/routing/subscription_manager.rs (a parked message could be overtaken
  by a later one) — per-subscriber FIFO, one in-flight slow send, rest queued
  behind it. Made the hardcoded 500 / 100 / 2s knobs configurable via
  `SubscriptionConfig` (`channel_capacity` / `max_parked_messages` /
  `slow_send_timeout` + builder methods). Exposed **pull** drop visibility as
  `dropped_messages() -> u64` on all subscriber types.

- **Drop-notification design decision (locked, implemented in step 2b):** the
  drop is *local* — between our routing actor and the user's consumer, NOT the
  network. We cannot and do not notify the network publisher (no such mechanism
  in MQTT 3.1.1; MQTT 5 Receive Maximum is the closest lever and only bounds the
  broker for QoS≥1). Who we *can* notify is the local consumer, two ways:
  - **pull** — cumulative `dropped_messages()` counter (shipped above); the
    metrics path and a complement to the push event below.
  - **push** — a `ReceiveEvent::Lagged { missed }` variant on the `receive()`
    return type (step 2b). `receive() -> Option<ReceiveEvent<M, E>>` with
    `Message` / `DecodeFailed` / `Lagged`, one type across all three layers
    (`Infallible` for the low layer). See step 2b for the full rationale.
  - **Rejected** two tempting shapes: (a) a `MessageMeta.lagged` field — it
    buries data loss in the happy path and is opt-in/easy to miss; (b) folding
    lag into `Err` (`Result<M, {Deserialize, Lagged}>`, broadcast-style) — the
    `while let Some(Ok(m))` idiom compiles unchanged from 0.2 and then silently
    ends the loop on the first (frequent) lag, it mislabels a healthy-stream
    notice as an error, and it breaks `TryStream` composition. The event enum
    makes the migration LOUD (old patterns fail to compile) and lag un-`Err`-able.

- **QoS≥1 caveat (why this matters):** rumqttc auto-acks incoming QoS≥1 publishes
  in its event loop *before* they reach our channel, so a backpressure drop
  silently breaks the delivery guarantee (the broker considers it delivered).
  Visibility (above) is the 0.3 mitigation; the real fix is manual-acks +
  Receive Maximum in 0.4.

- Full manual-acks/Receive-Maximum design → separate doc, implementation 0.4+.

### 2b. `ReceiveEvent` — the `receive()` return shape (push drop notice)

Completes §5's drop-visibility story (the push half; the pull counter shipped
in §5). A single event enum across all three receive layers:

```rust
#[non_exhaustive]
#[derive(Debug)]
pub enum ReceiveEvent<M, E> {
    Message(M),
    DecodeFailed(E),           // a message arrived but could not be decoded; stream continues
    Lagged { missed: u64 },    // `missed` messages dropped for this subscriber since the last report
}

impl<M, E> ReceiveEvent<M, E> {
    // Explicit, greppable opt-out: keep only messages.
    pub fn message(self) -> Option<M> { /* ... */ }
}
```

- `receive() -> Option<ReceiveEvent<M, E>>`; `None` still means the
  subscription is closed.
- Per layer (one type, coherent): low `Subscriber::recv` uses
  `ReceiveEvent<MessageType<T>, Infallible>` (the `DecodeFailed` arm is
  statically dead; the `#[non_exhaustive]` wildcard already covers it); mid
  `MqttSubscriber` uses `E = (Arc<TopicMatch>, F::DeserializeError)` (keep the
  topic available on payload failure); top `MqttTopicSubscriber` uses
  `E = MessageConversionError<DE>` (unchanged, stays a real `std::error::Error`).
- **Position: lagged is an EVENT, not an error.** The stream is healthy and the
  next buffered message is intact; `broadcast::RecvError::Lagged` is a `Result`
  only because `broadcast` has no `Option` termination channel, a constraint we
  don't share. Folding lag into `Err` was rejected (see §5).
- Implementation is simple and needs no actor-side marker injection: drops only
  happen when the consumer's channel is full, so a counter-delta check at the
  top of `recv()` (compare the `dropped_messages` atomic against a locally
  remembered `last_seen_drops`) is prompt by construction — the consumer cannot
  be parked on an empty channel while drops occur. `Subscriber` already holds
  the `Arc<AtomicU64>`.
- **Documented caveat (positional fuzziness):** the dropped messages logically
  follow whatever is still buffered ahead of the consumer, but the `Lagged`
  notice is emitted before that backlog drains. Exact positioning would require
  reserving channel slots for markers — not worth the complexity.
- Canonical consumer loop (docs + `examples/` should show this `match` form, not
  `while let Some(ReceiveEvent::Message(m))`, which re-creates the early-exit
  footgun). Breaking: every 0.2/early-0.3 `receive()` loop must be rewritten,
  and — deliberately — old `while let Some(Ok(m))` shapes fail to compile.
- Keep `dropped_messages()` as the cumulative metrics side channel.

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

1. De-leak API (§1) — **DONE 2026-07-09** (see PLAN_0.3_DELEAK.md for the
   commit list and design record).
2. Ordering-bug fix + backpressure knobs (§5) — **DONE 2026-07-09**.
   Per-subscriber FIFO (one in-flight slow send, rest queued behind it);
   `channel_capacity`/`slow_send_timeout`/`max_parked_messages` on
   `SubscriptionConfig` (+ builder methods); `dropped_messages()` on the
   subscriber types. Plan-critic + code-critic passed.
3. `ReceiveEvent` receive() shape + push lag notice (§2b) — **DONE 2026-07-09**.
   `Option<ReceiveEvent<M,E>>` (`Message`/`DecodeFailed`/`Lagged{missed}`,
   `#[non_exhaustive]`, `.message()` opt-out) across all three subscriber
   layers; lag via a counter-watermark in `Subscriber::recv` (`missed` exact,
   position approximate — documented). `IncomingMessage` alias renamed
   `SubscriberEvent`. Migrated all examples/README/comparison-doc/tests.
   Adversarial critic + 4-angle code review passed. Deferred (tracked in
   ROADMAP): concrete topic in `MessageConversionError`.
4. MessageMeta (§2) + macro work (builds on the §2b `receive()` shape). **← NEXT**
5. Connection state (§4).
6. SubAck minimal (§3) on whatever backend is current — QoS downgrade surfaces
   at `subscribe()`, not in the `receive()` stream.
7. Backend swap (§6) + tracked-notice publish/subscribe API — gated on the
   eagle coordination outcome; may slip to 0.4 without blocking the release.

## Open items (external)

- eagle's response to mqtt-typed-client-next#1 (coordination, QoS-downgrade
  PR, release cadence).
- LabOverWire/mqtt-lib#100 response (mqtt5 client/broker split — 0.4+ signal
  only).
- bytebeamio/rumqtt reports filed 2026-07-09 (both bugs verified on main @
  e886a78): issue #1056 + PR #1058 (collision-in-clean fix, v4+v5, tests;
  fork branch holovskyi/rumqtt:fix-clean-collision-livelock), issue #1057
  (subscribe pkid reuse — PR waits for maintainer direction on stash vs
  StateError). Watch for responses.
