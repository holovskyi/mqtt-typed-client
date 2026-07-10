# 0.3 Release Plan

*Drafted 2026-07-08. Theme: **deepen the delivery guarantees** — metadata,
connection observability — and redesign the public API so that
MQTT 5 (0.4) and the backend switch land later WITHOUT a second breaking
release. Everything here is v4/MQTT 3.1.1 functionally; every public type is
designed v5-first with v4 as the degenerate case.*

**Status note (2026-07-10):** ack surfacing (§3), resubscribe-failure surfacing
(§2c), and the backend swap (§6) were **DEFERRED TO 0.4** and MOVED OUT of this
file into [PLAN_0.4.md](./PLAN_0.4.md) — clean addressable SubAck correlation
needs the next backend, which is not yet on crates.io. 0.3 shipped upstream-only.
This file is now the historical record of what 0.3 actually delivered
(§1, §2, §2b, §4, §5 — all DONE).*

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
- **Reserve-and-error (collision policy, NARROW scope — Fork 2).** Today a
  pattern like `data/{payload}` with a `payload` field silently mis-binds (the
  field steals the body, the wildcard loses its value — no error). Fix: a hard
  compile error **only when a reserved name is used as a named wildcard AND is
  also a struct field of that name** — i.e. exactly the silent-misbind bug. The
  check lives INSIDE the reserved-field match arms (`analysis.rs:153-160`), where
  the field is known to exist: each arm scans the pattern for a same-named
  wildcard (mirror of the `is_topic_param` scan at `:163-167`) and errors if
  found. It is NOT a pattern-level pre-check — "is it also a field" is unknowable
  before categorisation. A bare `{topic}` wildcard with no `topic` field keeps
  compiling as a normal param (do NOT break it). `RESERVED_FIELD_NAMES` const
  drives the `extract_field_types` exclusion (must now also exclude `meta`).
  Anonymous `+` never collides (`param_name()` is `None`). Error text lists the
  three roles + suggests renaming the wildcard (e.g. `{meta_id}`).
- **Breaking note for CHANGELOG:** with narrow scope the break is only "reserved
  name used as BOTH a wildcard and a same-named field." For `payload`/`topic`
  that already silently mis-binds (erroring costs nothing). For `meta` it *works
  today* (meta-as-plain-param) — a real (near-zero-probability) semver break. One
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

**Implementation plan (phases a → b → c; critic-passed 2026-07-09 round 1).**
Grounded in the current tree (post §1/§2b/§5). Symbols verified present. Two
design forks resolved after the first critic pass (see "Resolved forks" below).

*Phase (a) — core plumbing.*

1. **Define the types** (new module, e.g. `core/src/message_meta.rs`;
   re-export at crate root next to `TopicMatch`/`ReceiveEvent`):
   ```rust
   #[non_exhaustive]
   #[derive(Debug, Clone)]
   pub struct MessageMeta {
       /// QoS of the delivered PUBLISH *packet* — NOT the subscription's
       /// granted QoS. With overlapping filters on one client the broker sends
       /// one packet at the highest matching granted QoS, so a QoS-0 subscriber
       /// can observe a higher value here (S4).
       pub qos: QoS,            // protocol-neutral QoS (mqtt_topic_engine::QoS), per §1
       pub retain: bool,
       pub dup: bool,
       pub v5: Option<Mqtt5Meta>,  // always None in 0.3
   }

   #[non_exhaustive]
   #[derive(Debug, Clone)]
   pub struct Mqtt5Meta {}         // empty stub in 0.3; fields land with the v5 backend
   ```
   - `#[non_exhaustive]` on both (LOCKED — reversibility argument confirmed:
     removing it later only relaxes, adding it later is breaking, so shipping
     WITH it is the conservative choice).
   - `Clone` required for the bare-type macro path (`Arc::unwrap_or_clone`).
   - `pub(crate) fn MessageMeta::v4(qos, retain, dup)` for the hot path.
   - **N5 (LOCKED):** ALSO a public `pub fn MessageMeta::new(qos, retain, dup)
     -> Self` (v5 = None). `non_exhaustive` otherwise forbids downstream users
     from constructing a `MessageMeta` in *their own* handler tests; a public
     ctor stays additive because v5 lives behind `Option`.

2. **Carry the raw metadata through the actor command.** Today
   `RawMessageType<T> = (String, T)` (`subscription_manager.rs:39`) and
   `dispatch_incoming_message(topic: String, data: T)` (`:689`) drop everything
   but topic+payload. Add `RawMeta { qos: QoS, retain: bool, dup: bool }` (map
   `rumqttc::QoS` → engine `QoS` via the existing `From` at `qos.rs:92`):
   - `RawMessageType<T> = (String, RawMeta, T)`.
   - `dispatch_incoming_message(topic, meta: RawMeta, data)` and the
     `Command::Send` variant carry it.

3. **Build `Arc<MessageMeta>` once per publish in `handle_send`.**
   (`:548`, beside `let data = Arc::new(data);` at `:571`.) Build
   `let meta = Arc::new(MessageMeta::v4(qos, retain, dup));` ONCE, then
   `Arc::clone(&meta)` per subscriber. Mirrors the payload-Arc fan-out.

4. **Widen the internal delivered tuple (routing layer stays a tuple).**
   `MessageType<T> = (Arc<TopicMatch>, Arc<T>)` (`:40`) →
   `(Arc<TopicMatch>, Arc<MessageMeta>, Arc<T>)`. Per-subscriber `message` at
   `:586` becomes `(topic_match, Arc::clone(&meta), Arc::clone(&data))`. Flows
   transparently through the channel and the low-level `Subscriber::recv`
   (`routing/subscriber.rs:125`, `ReceiveEvent<MessageType<T>, Infallible>`, no
   sig edit) and `message()` (`:80`). This tuple is internal (T = `Bytes` at the
   mid layer) — it is NOT the user-facing shape (see step 5).

5. **Mid layer: named structs, NOT wider tuples (FORK 1, LOCKED → option b).**
   `MqttSubscriber` is public and user-facing; do NOT widen its arms to tuples.
   Introduce a happy-path struct and a symmetric failure struct:
   ```rust
   #[derive(Debug)]
   pub struct IncomingMessage<T> {   // NOT non_exhaustive — meant to be destructured
       pub topic: Arc<TopicMatch>,
       pub meta: Arc<MessageMeta>,
       pub payload: T,
   }

   #[derive(Debug)]
   pub struct DecodeFailure<E> {     // NOT non_exhaustive; symmetric with above
       pub topic: Arc<TopicMatch>,
       pub meta: Arc<MessageMeta>,
       pub error: E,                 // E = F::DeserializeError at this layer
   }
   ```
   - **SF-1 (LOCKED): NOT `#[non_exhaustive]`.** Unlike `MessageMeta` (field-
     accessed), these structs EXIST to be destructured; `non_exhaustive` would
     force downstream `let IncomingMessage { topic, meta, payload, .. }` with a
     stray `..` and regress vs the old clean tuple destructure. Metadata growth
     is routed through `MessageMeta` (itself `non_exhaustive`), so these three
     fields are structurally complete — rely on additive-only discipline instead.
   - **SF-3 (LOCKED): error arm is a struct too**, `DecodeFailure<F::DeserializeError>`.
     The Fork-1 argument (a positional tuple makes the next field a breaking
     change) is symmetric — it applies to the error arm, so mirror it rather than
     ship a named/positional asymmetry. Meta IS available at the failure point
     (`client/subscriber.rs:88`).
   `SubscriberEvent<T, F>` (`client/subscriber.rs:21`) becomes
   `ReceiveEvent<IncomingMessage<T>, DecodeFailure<F::DeserializeError>>`.
   `receive()` (`:57`) destructures the low-level `(topic, meta, bytes)`,
   deserializes `bytes`, and yields `IncomingMessage { .. }` / `DecodeFailure { .. }`.
   Empty-payload `continue` arm (`:60`, `:80-86`) also updates its destructure;
   meta is dropped with the skipped retain-clear (correct, no leak).
   - NOTE meta-on-failure does not reach the typed top layer —
     `MessageConversionError<DE>` carries no meta, so it is a mid-layer-only
     affordance.

6. **Top layer: 3-arg trait + destructure the struct.**
   `FromMqttMessage::from_mqtt_message(topic, meta: Arc<MessageMeta>, payload)`
   (`structured/subscriber.rs:47`). `MqttTopicSubscriber::receive` (`:86`)
   destructures `IncomingMessage { topic, meta, payload }` and passes all three.

7. **Stop discarding `p.retain/qos/dup` in `async_client.rs`** (`:169-181`):
   pass them into `dispatch_incoming_message` via `RawMeta`.

8. **Fix the in-module tests** in `subscription_manager.rs`: the `handle_send`
   helpers (`:743`, `:766`, `:781` — `payload()` moves `msg.1` → `msg.2`) AND the
   §2b lag test destructures at `:910`/`:924` must adopt the 3-tuple + `RawMeta`.

*Phase (b) — macro (`macros/src/analysis.rs` + `codegen.rs`).*

1. **Recognise the `meta` magic field.** Add a `"meta"` arm beside
   `"payload"`/`"topic"` in field categorisation, with a `has_meta_field` flag +
   its (Arc-adaptive) type.
2. **Arc-adaptive type for `topic` + `meta`.** Extend the syntactic
   `is_arc_topic_match_type` into a per-field bare-vs-`Arc<_>` helper. Codegen:
   `Arc<_>` → move as-is; bare → `Arc::unwrap_or_clone(arc)` (MSRV 1.85.1 ≥ 1.76,
   confirmed). Applies to `topic`/`meta` only.
3. **Reserve-and-error — NARROW scope (FORK 2, LOCKED → option a).** Error ONLY
   when a reserved name (`payload`/`topic`/`meta`) appears as a named wildcard
   AND as a struct field of the same name — i.e. the actual silent-misbind bug.
   The check lives INSIDE the reserved-field match arms (`analysis.rs:153-160`),
   where the field is known present; each arm scans the pattern for a same-named
   wildcard (mirror of `is_topic_param` at `:163-167`). NOT a pattern-level
   pre-check (SF-2 — "is it also a field" is unknowable before categorisation).
   A bare `{topic}` wildcard with no `topic` field keeps compiling as a normal
   param (`extract_field_types` excludes the name, `analysis.rs:215`; do NOT
   break it). `RESERVED_FIELD_NAMES` const still drives that exclusion (now incl.
   `meta`). Anonymous `+` never collides (`param_name()` is `None`). Error text
   lists the roles + suggests a `{meta_id}`-style rename.
4. **Codegen wiring** (`codegen.rs`): `generate_from_mqtt_impl` emits the 3rd
   `meta` arg; `generate_subscriber_field_assignments` pushes `meta,` beside
   `payload,`/`topic,`. Fully-qualify `MessageMeta` (like `TopicMatch`).
5. **Tests** (trybuild/UI + a runtime example): narrow-collision errors (name as
   BOTH wildcard and field) for all three; bare `{topic}`-with-no-field still
   compiles; bare vs `Arc<MessageMeta>` both compile; no-`meta`-field struct
   unaffected.

*Phase (c) — examples/docs.*

1. One example reading `message.meta.qos` / `.retain`, showing both bare and
   `Arc<MessageMeta>` forms.
2. **CHANGELOG breaking section (expanded per critic B1):**
   (i) new `MessageMeta` feature; (ii) mid-layer `MqttSubscriber::receive` now
   yields `IncomingMessage<T>` instead of `(Arc<TopicMatch>, T)` — a REQUIRED
   migration for direct (non-macro) subscriber users; (iii) `{meta}` reserved as
   a wildcard when a `meta` field is also present (near-zero probability).
3. **Migrate every existing mid-layer consumer** (NOT just "an example"):
   `README.md:294`, `examples/100_all_serializers_demo.rs:92`,
   `tests/serializers_integration.rs:156`, the `core/src/lib.rs:58` doc example.
4. Doc the recommended default (`meta: Arc<MessageMeta>` = zero-copy).

**Resolved forks & fixes (two critic passes, 2026-07-09):**
- **Fork 1 — mid-layer shape:** named `IncomingMessage<T>` struct (and symmetric
  `DecodeFailure<E>`), NOT tuples. Reason: the surface is broken by §2b already;
  structs make the next metadata field a non-breaking add and are greppable.
- **Fork 2 — reserve-and-error scope:** NARROW (error only when a reserved name
  is both a wildcard and a same-named field). Reason: keeps a bare `{topic}` param
  that compiles today compiling.
- **SF-1 (round 2):** `IncomingMessage`/`DecodeFailure` are NOT `#[non_exhaustive]`
  — they are meant to be destructured; `non_exhaustive` would force a stray `..`
  and regress the destructure. Metadata grows inside `MessageMeta`, not here.
- **SF-2 (round 2):** narrow reserve-and-error check lives in the reserved-field
  match arms, not a pattern-level pre-check (corrected in the prose + phase b).
- **SF-3 (round 2):** error arm is a struct too (`DecodeFailure`), not a tuple —
  same anti-fragility argument as Fork 1, applied symmetrically.
- **S4:** `MessageMeta.qos` documented as the delivered-packet QoS.
- **`#[non_exhaustive]`:** LOCKED yes on `MessageMeta`/`Mqtt5Meta` (field-accessed;
  reversibility argument), NO on `IncomingMessage`/`DecodeFailure` (destructured).
- **`Mqtt5Meta {}` empty stub:** kept; no clippy empty-struct lint fires. Weakly
  motivated but harmless and reserves the visible shape.

### 3. Ack surfacing → MOVED to [PLAN_0.4.md](./PLAN_0.4.md) §3

Deferred to 0.4 (needs the `rumqttc-v4-next` backend for clean addressable SubAck
correlation). Full design lives in PLAN_0.4.md.

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

### 2c. Resubscribe-failure surfacing → MOVED to [PLAN_0.4.md](./PLAN_0.4.md) §2c

Deferred to 0.4 (gated on §3 SubAck surfacing on the reconnect path, which needs
the next backend). Full three-defect analysis + the `ReceiveEvent::SubscriptionLost`
design lives in PLAN_0.4.md.

### 6. Backend switch to rumqttc-v4-next → MOVED to [PLAN_0.4.md](./PLAN_0.4.md) §6

Deferred to 0.4 (adopt-with-mitigations; hard blocker = the next backend reaching
crates.io). Full migration recipe, feature-flag strategy, and fallback ladder
live in PLAN_0.4.md.

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
4. MessageMeta (§2) + macro work (builds on the §2b `receive()` shape) —
   **DONE 2026-07-09**. `MessageMeta`/`Mqtt5Meta`/`RawMeta`; routing tuple →
   `(Arc<TopicMatch>, Arc<MessageMeta>, Arc<T>)` built once in `handle_send`;
   mid-layer named structs `IncomingMessage<T>`/`DecodeFailure<E>` (breaking for
   direct subscribers, macro users unaffected); `FromMqttMessage` +`meta` arg.
   Macro recognises `meta`, Arc-adaptive `topic`+`meta` (bare →
   `Arc::unwrap_or_clone`, needed `#[derive(Clone)]` on `TopicMatch` in the
   engine), narrow reserve-and-error. Example `009_message_metadata` compile-
   tests the owned path. Two plan-critic passes + one code-critic pass (caught a
   blocker: bare `topic` needed `TopicMatch: Clone`).
5. Connection state (§4). **DONE 2026-07-10** — detailed plan/record in
   [PLAN_0.3_CONNSTATE.md](./PLAN_0.3_CONNSTATE.md). Shipped in two commits:
   the zombie-consumer bugfix (terminal event-loop death now runs the same
   channel cleanup as `shutdown()` via a new `Command::Shutdown`, so `receive()`
   yields `None` instead of hanging) then the feature
   (`MqttClient::connection_state() -> watch::Receiver<ConnectionState>`;
   `Connected{session_present}`/`Reconnecting{attempt}`/`Disconnected{reason}`,
   own `#[non_exhaustive]` enums; frozen-seed `state_rx` so late subscribers see
   the terminal; two-counter reconnect bookkeeping). Example `010`. Two critic
   passes (plan + code). No `mqtt-topic-engine` change.
6–8. **DEFERRED TO 0.4** — SubAck surfacing (§3), resubscribe-failure surfacing
   (§2c), and the backend swap (§6). Moved to [PLAN_0.4.md](./PLAN_0.4.md), which
   has its own order of work (backend swap first, then §3, then §2c).

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
