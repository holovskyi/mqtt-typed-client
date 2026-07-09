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

**Macro surface — FINAL (locked 2026-07-09, unbiased-agent-reviewed):**
magic-name field `meta` + Arc-adaptive field type + reserve-and-error.

- **Magic name (variant A).** An optional field named `meta` in the topic
  struct is recognised and auto-populated, exactly like `topic`. Non-declaring
  users pay nothing (no `meta` field → codegen never touches it). Rejected:
  by-type recognition (syntactically fragile), attributes-everywhere (breaks
  every existing struct or splits conventions). `meta` is currently an
  `Unknown fields` compile error, so adding it breaks nothing — except the
  `{meta}`-as-wildcard case (see reserve-and-error).
- **Arc-adaptive field type.** The macro accepts BOTH `meta: MessageMeta` and
  `meta: Arc<MessageMeta>` (same rule extended to `topic`: `TopicMatch` or
  `Arc<TopicMatch>`). Detection is syntactic (already done by
  `is_arc_topic_match_type`). `Arc<_>` → move (zero-copy, the recommended/doc'd
  default, mirrors the shared fan-out); bare → `Arc::unwrap_or_clone` (free when
  a subscriber is alone, deep-clone otherwise — an opt-in ergonomic cost the
  user chooses). Adaptation applies ONLY to `topic`/`meta` — the two values that
  physically arrive as a shared `Arc<_>` in the fan-out. `payload` and `{param}`
  are always freshly-owned per subscriber (deserialize / `FromStr`), so there is
  nothing to share and no Arc option for them.
- **Reserve-and-error (collision policy, uniform for all three names).** Today a
  pattern like `data/{payload}` with a `payload` field silently mis-binds (the
  field steals the body, the wildcard loses its value — no error). Fix: a hard
  compile error, checked at the *pattern* level (top of `analyze()`, before
  field categorisation), if any reserved name (`payload`/`topic`/`meta`) is used
  as a named wildcard. `RESERVED_FIELD_NAMES` const reused in the
  `extract_field_types` exclusion (must now also exclude `meta`). Anonymous `+`
  wildcards can never collide (`param_name()` is `None`). Error text lists the
  three roles + suggests renaming the wildcard (e.g. `{meta_id}`).
- **Breaking note for CHANGELOG:** unlike `{payload}`/`{topic}` (already
  silently broken, so erroring costs nothing), `{meta}` as a wildcard *works
  today* — reserving it is a real (if near-zero-probability) semver break. One
  CHANGELOG line; acceptable pre-1.0 with a loud error + trivial rename.
- **Escape-hatch attribute (`#[mqtt_meta] other: MessageMeta`) DEFERRED** as
  YAGNI: the user controls the pattern and can always rename a wildcard. Add
  only if a real user is forced to keep a `{meta}` segment by an external
  contract. The reserved-name error should hint at "rename the wildcard".
- **Plumbing (mechanical).** `MessageType<T>` becomes
  `(Arc<TopicMatch>, Arc<MessageMeta>, Arc<T>)`; `Arc<MessageMeta>` is built
  ONCE in `handle_send` (identical for all subscribers of a publish, shared like
  the payload Arc). `FromMqttMessage::from_mqtt_message` gains a third arg
  `meta: Arc<MessageMeta>`. `MessageMeta` re-exported at the crate root (macro
  emits a fully-qualified path, like `TopicMatch`). Stop discarding
  `p.retain/qos/dup` in async_client.rs. `Mqtt5Meta` behind `Option` (Box it if
  it grows) so carrying meta stays cheap on v4.
- NOTE: the backpressure lag notification is NOT a `MessageMeta` field — it is a
  `ReceiveEvent::Lagged` variant on the `receive()` return (see §5 / step 2b),
  landed before this section. `MessageMeta` is only per-message protocol
  metadata.

### 3. Ack surfacing

- **SubAck (minimal, backend-independent):** stop dropping
  `SubAck.return_codes` — surface broker-side subscription rejection and QoS
  downgrade (log + typed event/error to the subscriber).
- **Must cover the RECONNECT path, not only `subscribe()`** (from r/rust
  feedback, verified 2026-07-09). The initial-`subscribe()` surfacing and the
  resubscribe-after-reconnect surfacing read the same `SubAck.return_codes`;
  scoping SubAck to `subscribe()` only (as the order-of-work step 6 note below
  says) leaves the reconnect hole open. Honest resubscribe-failure detection
  (§2c) *requires* this — so §3-on-the-reconnect-path GATES §2c.
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
- **Zombie-consumer bug (from r/rust feedback, verified 2026-07-09):** on
  terminal death (`async_client.rs:198-219` `break`) the subscriber channels are
  NOT closed — cleanup only runs on explicit `MqttConnection::shutdown()`
  (`subscription_manager.rs` cleanup path), so every consumer parks on
  `receive().await` forever instead of getting `None`. Making the state
  *observable* via the watch channel does not fix this: existing consumer loops
  still hang. Fix: on terminal death, run the same channel-cleanup path as
  `shutdown()` so `receive()` yields `None` and every consumer loop terminates.
  Small, and it is the difference between "observable" and "actually correct".
- **Negative decision (locked):** `ConnectionState` does NOT carry resubscribe
  failure. "The connection is up but 3 of 7 subscriptions did not come back" is
  a property of the *subscription*, not the connection — it belongs on the
  affected subscriber's `receive()` stream (see §2c), not on this channel.
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

### 2c. Resubscribe-failure surfacing (from r/rust feedback, verified 2026-07-09)

**The real reconnect gap — not previously in any plan.** After a session-less
reconnect we call `resubscribe_all()`, but its result is invisible and never
acted on. Three stacked defects (`subscription_manager.rs:479-511`,
`async_client.rs:153-158`):

1. **The `Ok` arm is a lie.** rumqttc's `client.subscribe().await == Ok(_)` means
   "enqueued onto the event-loop channel", NOT "broker accepted". A broker that
   rejects the subscription (SUBACK `0x80`) or silently downgrades QoS produces
   `Ok` here. So `failed_topics` only ever catches `ClientError` (channel
   closed/full) — i.e. the client is already dead. The failure that matters
   (broker refuses the resubscribe) is completely invisible. **Cannot be fixed
   without §3 SubAck surfacing on the reconnect path** — that gates this section.
2. **The error carries no data.** `failed_topics` is collected then discarded;
   the function returns the unit `SubscriptionError::ResubscribeFailed`. Which
   topics failed is lost.
3. **Nobody consumes it.** `async_client.rs` does `.inspect_err(|e| error!(...))`
   and continues — no retry, no state change, no notification. An affected
   `Subscriber<T>` looks healthy and simply never receives another message.

**Home: the `ReceiveEvent` enum (§2b), NOT `ConnectionState`.** It is already
`#[non_exhaustive]`, already carries this class of "stream alive but you lost
something" event (`Lagged`), is delivered to exactly the affected subscriber,
and has no external users yet. Add a variant (name a strawman):

```rust
ReceiveEvent::SubscriptionLost { reason: ... }  // broker refused to restore this subscription
```

**Mechanical prerequisite:** `get_topics_for_resubscribe()`
(`mqtt-topic-engine/src/topic_router.rs:278`) returns `HashMap<ArcStr, QoS>` —
pattern→QoS with NO reverse mapping to the subscriber IDs that must be notified.
`TopicRouter` has the data (`self.subscriptions`), it just isn't returned.
Changing that return type **touches `mqtt-topic-engine`** (published standalone
→ version bump).

**Retry policy (open, lean (a)):** (a) mark the subscription lost + notify;
(b) bounded retry with backoff then notify. Lean (a) for 0.3 — without SubAck
confirmation a retry cannot tell success from failure, so it is just a louder
no-op. At minimum: do not silently continue.

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
- This is also what answers the user-facing "what happens to a publish issued
  mid-outage?" question: inflight QoS 1/2 replay and offline queueing live in
  rumqttc's `EventLoop`/`state.rs`, and the `-next` audit
  (`research/RUMQTTC_NEXT_AUDIT_2026.md:25-28`) confirms reconnect retransmission
  with notice senders preserved + `SessionReset` on session loss. On upstream
  rumqttc today we cannot honestly state the outcome; §6 makes it answerable. (An
  own managed offline queue stays out of scope — see the standing negative
  decision in `research/CLIENT_LIBRARY_LANDSCAPE_2026.md`.)

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
   at `subscribe()` AND on the reconnect/resubscribe path (§3 covers both).
7. Resubscribe-failure surfacing (§2c) — `ReceiveEvent::SubscriptionLost` on the
   affected subscriber; gated on step 6 (SubAck on the reconnect path). Touches
   `mqtt-topic-engine` (return-type change → version bump).
8. Backend swap (§6) + tracked-notice publish/subscribe API — gated on the
   eagle coordination outcome; may slip to 0.4 without blocking the release.

## Open items (external)

- eagle's response to mqtt-typed-client-next#1 (coordination, QoS-downgrade
  PR, release cadence).
- LabOverWire/mqtt-lib#100 — RESOLVED 2026-07-09: author opened PR #101 same
  day, broker gated behind `broker` feature (default-on), ships as mqtt5
  0.36.0. Verified client-only build (158→111 crates, broker subtree gone,
  cargo check clean). 0.4+ signal = positive. TODO: post a thank-you comment
  confirming the test (draft ready; not yet posted).
- bytebeamio/rumqtt reports filed 2026-07-09 (both bugs verified on main @
  e886a78): issue #1056 + PR #1058 (collision-in-clean fix, v4+v5, tests;
  fork branch holovskyi/rumqtt:fix-clean-collision-livelock), issue #1057
  (subscribe pkid reuse). **#1057 update 2026-07-09:** answered by the
  *rumqttc-next fork author* (thehouseisonfire), NOT a bytebeamio maintainer —
  he confirmed the bug (hit it himself during spec-compliance checks) and his
  fork already fixes it by linearly scanning the 2¹⁶ pkid space for a free id
  (a third option beside our stash/StateError suggestions), with a plan for
  something more elegant under pressure. Implication: another point for §6
  (rumqttc-next already fixes BOTH #1056 and #1057, upstream fixes neither).
  Upstream-PR direction still formally open (bytebeamio unresponded); OPEN
  DECISION for Artem: send an upstream PR mirroring the linear-scan approach vs
  keep waiting on a bytebeamio maintainer. Watch for responses.
