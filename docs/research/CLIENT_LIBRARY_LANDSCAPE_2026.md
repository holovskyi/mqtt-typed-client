# MQTT Client Library Landscape Research (July 2026)

*Internal research document. A survey of MQTT client library design across ~10 language
ecosystems plus adjacent typed pub/sub systems, and an analysis of which approaches are
worth adopting in `mqtt-typed-client`. Produced with parallel research agents covering:
Haskell/OCaml/typed FP; F#/.NET/Scala/Kotlin; Erlang/Elixir/Go/Swift; TypeScript/Python;
and cross-protocol systems (AsyncAPI, Zenoh, NATS, ROS 2, Kafka) + the Rust ecosystem.*

---

## 1. Executive summary

**The core bet of this library is validated everywhere we looked.** No surveyed library
in any ecosystem — including Haskell (which has the type-level machinery), TypeScript
(which has template literal types), and Go/JVM/.NET (which have the largest MQTT user
bases) — combines all three of:

1. named, **typed** topic parameters checked at compile time,
2. fan-out routing of one broker stream to multiple typed subscribers, and
3. pluggable per-topic payload serialization.

Each idea exists separately, untyped and at runtime (MQTTnet `TopicTemplate`,
`mqtt-pattern` in JS, Tortoise's topic-list pattern matching in Elixir, paho routers in
Go), but the combination is unoccupied territory. An accurate elevator pitch that came
out of the FP survey: **"Servant for MQTT topics, in Rust."**

Equally consistent across ecosystems is where we are behind: **delivery semantics.** The
best clients (HiveMQ Java, autopaho, MQTTnet, emqtt, net-mqtt) treat ack surfacing,
consumer-driven flow control, session/queue persistence, and MQTT 5 as core features.
Our type safety currently ends at topic construction; the strongest next moves deepen
the *guarantees*, not the surface area.

Recurring patterns that showed up independently in 3–5 ecosystems (strong signal):

| Pattern | Seen in |
|---|---|
| Manual/deferred acks as consumer-driven backpressure (ack ⇄ Receive Maximum) | HiveMQ Java, HiveMQ .NET (`WithManualAck`), emqtt (`auto_ack: false`), Alpakka (`MqttMessageWithAck`), gmqtt (handler return = PUBACK code), NATS JetStream (AckExplicit) |
| Typed request/response over ResponseTopic + CorrelationData, as a *layer* | MQTTnet.Extensions.Rpc, paho.golang `extensions/rpc`, net-mqtt-rpc, minimq, NATS `request()`, AsyncAPI 3.0 `reply` |
| Topic aliases as an invisible client-side optimization, never API surface | HiveMQ Java, MQTT.js, emqtt |
| Connection state as an observable value/callback, not just logs | autopaho (`OnConnectionUp/Down`), mktt (`StateFlow`), Tortoise (`connection(:up/:down)`), NestJS (`client.status`) |
| Publish/subscribe results carrying broker reason codes instead of throwing | MQTTnet v5, paho v2 (`ReasonCode` types), net-mqtt (`[Either SubErr QoS]`), autopaho |
| Pluggable persistence interfaces (session state vs outbound queue, separately) | autopaho (`Session` + `Queue`), MQTT.js (`outgoingStore`), paho classic (`Store`), DitchOoM (SQLite) |

---

## 2. Ecosystem survey highlights

### 2.1 Haskell / OCaml / typed FP

- **net-mqtt** (the Haskell flagship, MQTT 3.1.1 + 5.0): the ecosystem's one big
  type-safety idea is the **`Topic` vs `Filter` split** — distinct validated newtypes, so
  publishing to a wildcard filter is unrepresentable. Callback *ordering* is an explicit
  user choice (`SimpleCallback` = task-per-message/unordered vs `OrderedCallback` =
  inline/backpressures TCP vs `LowLevelCallback` = full `PublishRequest` with
  retain/dup/qos + v5 properties). `subscribe` returns per-filter broker verdicts;
  QoS 1/2 publish parks on an STM channel until PUBACK/PUBCOMP.
- **SubTree** (net-mqtt's trie): wildcard lookup is a **monoidal fold** over the trie
  (`findMap :: Monoid m => Topic -> (a -> m) -> SubTree a -> m`) — QoS aggregation,
  subscriber-set dedup, and metrics are all instances of one lookup with different
  monoids. lpeterse's dormant `mqtt` crate adds **filter subsumption** (`matchFilter`:
  is filter A covered by filter B) — enables skipping redundant broker SUBSCRIBEs.
- **grpc-mqtt** (Awake Security, production): when Haskellers wanted typed pub/sub over
  MQTT they reached for protobuf codegen. Ships **packetization** (split/reassemble over
  a size limit) and opt-in **stream batching** — proven answers to our large-payload and
  throughput items.
- **OCaml**: far behind (no MQTT 5 anywhere). One nice touch in hyper-systems/ocaml-mqtt:
  connect takes a **host list for failover**.
- **fs2-mqtt** (Scala): session as `Resource` (lifecycle = scope), backpressure inherent
  to pull-based streams, and **reconnect policy as a composable value**
  (`limitRetries(5).join(fullJitter(2s))` via cats-retry) instead of numeric knobs.

### 2.2 .NET / JVM

- **MQTTnet**: `PublishAsync` returns `MqttClientPublishResult` (completes after
  PUBACK/PUBCOMP, carries reason code; broker-side failure is a value, not an
  exception). `TopicTemplate` extension = runtime, string-only version of our macro,
  **explicitly aligned with AsyncAPI channel-address syntax**. `Extensions.Rpc` =
  awaitable request/response with pluggable correlation, works even on 3.1.1.
  **Cautionary tale:** the v4 `ManagedMqttClient` (offline queue + auto-reconnect) was
  *deleted* in v5 because its ack semantics for queued messages were never crisp — an
  offline queue must not pretend to be a publish.
- **HiveMQ Java client**: best-in-class backpressure — Reactive Streams demand wired to
  **MQTT Receive Maximum**, withholding PUBACKs when consumers lag, **QoS-tiered**
  (QoS 0 droppable under pressure, QoS 1/2 never). Subscribe returns
  `FlowableWithSingle<SubAck, Publish>` — the SUBACK and the message stream in one
  value. Automatic transparent topic aliasing. Three convertible typed views of one
  client (blocking/async/reactive).
- **Kotlin ecosystem**: mktt exposes connection state as `StateFlow<MqttConnectionState>`;
  ktor-mqtt's `publish()` suspends until the full QoS handshake and returns a response;
  DitchOoM backs the in-flight/offline queue with **transactional SQLite** — the credible
  version of the managed-client idea. Nobody types topics.
- **F#**: no native client; the community hand-writes DU-per-topic-family decoding —
  i.e., manually writing exactly what our macro generates.

### 2.3 Erlang / Go / Swift

- **emqtt** (EMQX): messages are maps with **full metadata** (qos/retain/dup/properties).
  The fullest MQTT 5 surface surveyed (aliases, Receive Maximum, subscription
  identifiers, shared subscriptions, enhanced auth, QUIC transport). Key idea:
  **`{auto_ack, false}`** — the app acks QoS 1/2 explicitly; combined with Receive
  Maximum this *is* consumer-driven flow control.
- **Tortoise311** (Elixir): topic delivered as a list of levels so pattern matching is
  the router; callbacks return declarative `next_actions` instead of calling the client
  (solves reentrancy); `subscription/3` callback surfaces per-filter SUBACK results
  including QoS-downgrade warnings.
- **paho classic (Go)**: the **Token** pattern (`Wait/Done/Error` handle from every
  operation — call site picks sync or async). Documented footgun we structurally avoid:
  "handlers must not block" or keepalive starves.
- **autopaho (Go, current gen)** — the library to study hardest for reliability:
  deliberately layered (protocol client below, `ConnectionManager` above) with **two
  separate pluggable persistence interfaces** — session state store (inflight QoS
  packets) vs outbound queue — and a **dual publish API**: `Publish` (blocks until
  PUBACK, returns reason code) vs `PublishViaQueue` (fire-and-forget, queue absorbs
  offline periods, survives restarts). `OnConnectionUp(connack)` /
  `OnConnectionDown() -> bool` (return false = stop retrying). Notably it does **not**
  auto-resubscribe: with MQTT 5 sessions the right behavior depends on CONNACK's
  Session-Present, so it hands the decision to the user.
- **Swift mqtt-nio v3 (unreleased)**: subscription as an AsyncSequence **scoped to a
  closure with automatic UNSUBSCRIBE on scope exit** — structured-concurrency spelling of
  our subscriber-drop semantics. The v2 client namespaces MQTT 5 as `client.v5.*` — a
  clean way to add v5 without breaking the v3 surface.

### 2.4 TypeScript / Python

- **The niche is empty there too**: no template-literal-typed MQTT library on npm, no
  zod+MQTT package; fastapi-mqtt — despite living inside FastAPI — does *not* extract
  `{id}` params from topics.
- **aiomqtt** — the most instructive design conversation: earlier versions had
  per-pattern filtered streams (≈ our typed subscribers) and the maintainers **removed
  them** in favor of one message iterator + `topic.matches()`, citing hidden per-filter
  queues and unclear slow-consumer behavior. Their retreat is the strongest argument
  that our trie router is the hard, differentiating part — and that its queueing/overflow
  policy must be explicit and documented. Their backpressure design is minimal and
  honest: bounded inbound queue, drop-with-warning, **pluggable queue class** (e.g.
  priority queue).
- **gmqtt**: async handler's *return value* becomes the PUBACK reason code —
  at-least-once *processing*, not just delivery. MQTT 5 properties as plain kwargs.
- **MQTT.js**: pluggable `outgoingStore`/`incomingStore` (memory/LevelDB/localForage);
  automatic topic-alias assignment; `reconnectOnConnackError` knob.
- **paho-mqtt v2 lesson**: unifying v3/v5 callback signatures *after the fact* broke the
  entire ecosystem (mandatory `CallbackAPIVersion`). Design one metadata/reason-code
  surface now, with v3.1.1 as the degenerate case.
- **ngx-mqtt**: reference-counted subscriptions (SUBSCRIBE on first observer,
  UNSUBSCRIBE on last) and an `observe` vs `observeRetained` split — retained-replay
  semantics as a per-subscription choice.
- **Sparkplug B**: compression as a **self-describing per-publish option** (payload
  wrapped in an envelope naming the algorithm) rather than a connection setting.

### 2.5 Cross-protocol and Rust ecosystem

- **AsyncAPI**: channel `address` with `{parameter}` placeholders + parameter schemas is
  a near-exact declarative twin of `#[mqtt_topic]`; MQTT bindings cover LWT, QoS,
  expiry, correlation, response topics; AsyncAPI 3.0 makes **request/reply first-class**.
  Official tooling has **no Rust client generator** (Modelina generates models only).
  The "AsyncAPI ⇄ Rust typed MQTT client" niche is empty in both directions; our macro
  already contains everything needed to *emit* a spec ("utoipa of MQTT").
- **Zenoh**: RFC #2510 proposes typed publisher/subscriber wrappers over an untyped core
  — independent validation of our architecture — plus **schema-version tags in message
  attachments** for fast mismatch rejection (maps to MQTT 5 user properties).
  `zenoh-ext`'s AdvancedPublisher/Subscriber show reliability (sequence numbers, miss
  detection, retransmission) shipped as an **opt-in layer**, not client bloat.
- **NATS**: `client.request()` ergonomics is the bar for RPC; JetStream **pull
  consumers** = backpressure by construction with explicit ack policies; the `micro`
  services framework (typed endpoints + auto-published stats) has no MQTT equivalent.
- **ROS 2**: named **QoS profiles** (preset bundles like `SensorData`) instead of raw
  knobs scattered at call sites.
- **Kafka Schema Registry**: schema ID stamped into each message + compatibility
  enforced at registration (CI gate, not runtime surprise). MQTT 5 user properties +
  content-type give us the transport hooks Kafka had to invent.
- **Rust competitors**: rumqttc has a feature-gated `v5` module (steadily improving) and
  — critically — a **`manual_acks` mode**, the primitive our backpressure design needs.
  paho-mqtt-rust: full v5 but C-bindings friction, untyped. **`mqtt5` crate**: active
  (v0.35.1, 2026-07), v5-native client+broker with shared-subscription parsing —
  untyped, but it sets the v5 feature bar. *(Correction 2026-07-08: the "RPC
  utilities" previously claimed here don't exist in the client crate — request/response
  is only a wasm example. Full evaluation as a candidate backend:
  [MQTT5_CRATE_EVALUATION_2026.md](./MQTT5_CRATE_EVALUATION_2026.md).)* Embedded: **minimq**
  (no_std, v5, built-in request/response) and **miniconf** (derive macro mapping a
  settings struct onto topic paths — the closest existing "typed topics" relative in
  Rust). A future no_std story implies a sans-IO core (cf. `mqtt-protocol-core`), since
  rumqttc is tokio-bound. Typed-topic competition on crates.io: none found.

---

## 3. What to adopt — ranked

### Tier 1 — deepen the guarantees (highest value, mostly maps to existing ROADMAP items)

1. **Ack surfacing for publish and subscribe.**
   - `publish()` for QoS 1/2 should resolve when the broker confirms, returning a
     result carrying the reason code (MQTTnet `MqttClientPublishResult`; net-mqtt's
     packet-id → oneshot map is the implementation sketch). Broker-side failure is a
     *value*, not a panic/exception.
   - `subscribe()` should surface the SUBACK verdict — granted QoS or per-filter error —
     ideally as part of the same return value (HiveMQ's `FlowableWithSingle<SubAck,
     Publish>` → Rust spelling: `subscribe()` resolves to `(SubAck, Subscriber<T>)` or
     a subscriber with an `.ack()` accessor). Tortoise's QoS-downgrade *warning* is a
     detail worth keeping.
   - Consider a Token/handle-style escape hatch (paho Go): fire-and-forget callers just
     drop the future.

2. **Incoming message metadata (retain/qos/dup + properties).**
   Two proven spellings: a parallel API (`subscribe_with_meta()` yielding
   `(T, MessageMeta)` — net-mqtt's LowLevel variant; keeps the common path clean) or a
   macro-driven one — since we already auto-populate `topic: Arc<TopicMatch>`, an
   optional `meta: MessageMeta` field in the topic struct is the natural extension.
   Design the `MessageMeta`/reason-code surface **once, v5-ready**, with v3.1.1 as the
   degenerate case (paho v2's painful lesson). ngx-mqtt's retained-replay-per-
   subscription choice belongs here too.

3. **Backpressure with an explicit slow-consumer policy.**
   The convergent industry design: bounded per-subscriber channels + **deferred acks
   propagating to the broker** via Receive Maximum (HiveMQ, emqtt, JetStream). rumqttc's
   `manual_acks` mode is the primitive. Make the overflow policy an explicit,
   documented per-subscription choice (drop-oldest / drop-newest / block — QoS-tiered:
   QoS 0 droppable, QoS 1/2 never silently dropped). aiomqtt's pluggable queue class
   (priority queues) is a cheap extra. This is *the* place where being explicit
   differentiates us — aiomqtt retreated from per-pattern streams precisely because
   these semantics were fuzzy.

4. **MQTT 5 support (via `rumqttc::v5`, feature-gated).**
   Unlocks everything in Tier 2. Namespacing idea if API tension arises: mqtt-nio's
   `client.v5.*`. Surface properties ergonomically (gmqtt's kwargs ≈ our builder /
   publish-options struct; NestJS's `MqttRecordBuilder` precedent).

5. **Connection state as an observable value.**
   A `watch`-able `ConnectionState` (mktt's `StateFlow`, autopaho's callbacks) +
   `OnConnectionDown() -> bool`-style "stop retrying" escape hatch. Also fixes the
   current silent-death of the event loop after `MAX_CONSECUTIVE_ERRORS`. With v5,
   consult CONNACK Session-Present before resubscribing (autopaho's argument) instead of
   always replaying.

### Tier 2 — differentiating extensions (where typed topics give us an unfair advantage)

6. **Typed RPC over MQTT 5** — the single biggest gap in the whole MQTT ecosystem, and
   the feature our macro is uniquely positioned to own:
   `#[mqtt_rpc(request = "devices/{id}/cmd", response = "devices/{id}/rsp")]` generating
   `call(&self, id, req: Req) -> Result<Resp>` (per-call correlation data, ephemeral
   response subscription, timeout) plus a typed responder trait on the server side.
   Reference implementations: paho.golang `extensions/rpc`, MQTTnet.Extensions.Rpc
   (including a v3.1.1 topic-convention fallback), net-mqtt-rpc (~50 lines — proof it
   layers cleanly *on top of* the client), NATS `request()` for ergonomics. Every
   surveyed implementation is untyped; ours wouldn't be.

7. **AsyncAPI export.** Emit an AsyncAPI 3.0 document from `#[mqtt_topic]` declarations
   (inventory-style registry or cargo subcommand; the `asyncapi` crate provides the
   model). Cheap, unique, buys docs/Studio/codegen-for-other-languages, and MQTTnet's
   TopicTemplate confirms our `{param}` syntax is already AsyncAPI-compatible. Import
   (AsyncAPI → generated topic structs) can come later.

8. **Shared subscriptions as a typed API.** `$share/{group}/…` is client-side syntax —
   cheap to add as `topic_client.subscribe_shared("workers")` returning the same typed
   subscriber. Router wrinkles to handle: incoming messages arrive on the *plain* topic;
   a shared and non-shared subscription to the same filter are distinct broker
   subscriptions (QoS aggregation must not merge them). Combined with manual acks (item
   3) this is the "work queue" story.

9. **`Topic` vs `TopicFilter` type split** (net-mqtt). Distinct validated types so a
   wildcard-bearing value can never reach `publish`. The macro can emit both; the
   low-level API gets smart constructors + `TryFrom<&str>`.

10. **Reconnect policy as a composable value** (fs2-mqtt/cats-retry): a `ReconnectPolicy`
    the user passes in (max retries, backoff, jitter — composable), with "fail loudly,
    let a supervisor restart" (net-mqtt's philosophy) as one policy value. Multi-host
    failover (ocaml-mqtt) slots in here.

11. **Topic aliases as an invisible optimization** (HiveMQ, MQTT.js, emqtt). Our typed
    publishers are the perfect alias-assignment unit: the client statically knows the
    topic universe; hot concrete topics get aliases (LRU per parameterized pattern)
    within the negotiated TopicAliasMaximum. Never API surface.

12. **QoS/behavior profiles** (ROS 2): named presets bundling qos+retain+expiry+alias
    policy — `#[mqtt_topic("...", profile = "telemetry")]` — instead of loose knobs at
    call sites.

### Tier 3 — longer-term / architectural

13. **Offline outbound queue with pluggable persistence — only with explicit ack
    semantics.** The decomposition to copy is autopaho's: session-state store (inflight
    QoS packets) and outbound queue as *two separate traits*, with a **dual publish
    API** (`publish()` = confirmed, `publish_queued()` = fire-and-forget into the
    durable queue). The anti-pattern to avoid is MQTTnet's deleted ManagedClient
    (ambiguity of "publish succeeded" for queued messages); DitchOoM's transactional
    SQLite store shows the credible implementation bar.

14. **Compression as a self-describing codec layer** (Sparkplug): composed with the
    serializer, declared per message via MQTT 5 content-type/user-properties (or an
    envelope on v3), per-publish opt-in. Plus grpc-mqtt-style **packetization** for
    payloads above a size limit, and optional small-message batching.

15. **Schema evolution hooks** (Kafka registry + Zenoh RFC): stamp a schema
    version/fingerprint into a user property; dispatch/deserialize tolerantly on
    receive. Even without a registry server, "fingerprint + compile-time compat tests"
    answers the question every serious deployment eventually asks.

16. **Router generalization** (SubTree): monoidal fold over trie matches — QoS max,
    subscriber sets, metrics counters as instances of one lookup; **filter subsumption**
    to skip redundant broker SUBSCRIBEs; verify reference-counted dedup of identical
    patterns (SUBSCRIBE on first, UNSUBSCRIBE on last — ngx-mqtt).

17. **Middleware/interceptor hook** in the router: `(topic, params, raw payload)` before
    deserialization — metrics, auth, tracing (NestJS guards/pipes, aedes authorize
    hooks, HiveMQ packet-level interceptors).

18. **Testing story**: an embeddable in-process broker for integration tests is a gap in
    Rust (rumqttd is closest); KMQTT/MQTTnet.Server/gomqtt show the value. A sans-IO
    protocol core (Pekko mqtt-streaming, `mqtt-protocol-core`) is the long-term enabler
    for both fake-transport testing and the no_std roadmap item.

### Explicit non-adoptions

- **Single-iterator-only dispatch** (aiomqtt): their simplification is the right call
  *for them*; our router is the product. But adopt their honesty about queue policy.
- **Managed-client offline queue without crisp semantics** (MQTTnet v4): don't.
- **One-global-callback dispatch** (net-mqtt, CocoaMQTT, ocaml-mqtt): the pattern we
  exist to replace.
- **Protocol-as-raw-flow as the user API** (Pekko mqtt-streaming): beautiful internals,
  wrong altitude for users; keep it as an internal/testing idea only.

---

## 4. Suggested sequencing

A pragmatic order, front-loading items that are (a) already on the roadmap, (b) small,
or (c) prerequisites:

1. **Message metadata** (Tier 1.2) — small, already roadmapped, unblocks retained-message
   ergonomics; design the v5-ready `MessageMeta` now.
2. **Ack surfacing** (Tier 1.1) — suback first (no rumqttc changes needed for basic
   surfacing), puback next.
3. **Connection state watch channel** (Tier 1.5) — small, high perceived quality.
4. **MQTT 5 behind a feature flag** (Tier 1.4) — the enabler for the differentiators.
5. **Backpressure/manual-ack design** (Tier 1.3) — needs a design doc; interacts with 2
   and 4.
6. **Typed RPC** (Tier 2.6) + **shared subscriptions** (Tier 2.8) — the headline
   features once v5 lands; RPC can ship as an extension crate first (net-mqtt-rpc
   proves the layering).
7. **AsyncAPI export** (Tier 2.7) — independent of the above, high marketing value,
   can proceed in parallel.
8. Tier 3 as demand appears.

---

## 5. Positioning notes (for README / comparison docs)

- "Servant for MQTT topics, in Rust" — accurate and legible to FP-aware audiences.
- The typed-topic + typed-payload + router combination has **no competitor in any
  surveyed ecosystem**; cite the closest relatives honestly (MQTTnet TopicTemplate,
  mqtt-pattern, Tortoise pattern matching, miniconf) — all runtime and/or untyped.
- Our channel-per-subscriber dispatch *structurally* avoids the "handler must not block
  or the connection dies" footgun that paho Go/net-mqtt/Tortoise document as warnings.
- Ecosystem gaps worth naming where relevant: no maintained ZIO or F# native client, no
  Kotlin coroutines flavor of HiveMQ, no MirageOS client, empty TS template-literal
  niche — typed MQTT is underserved everywhere, which supports the thesis, and several
  of this document's ideas (AsyncAPI export especially) make our types consumable beyond
  Rust.
- Watch: `mqtt5` crate (v5 feature bar in Rust), miniconf/minimq (embedded typed-topic
  demand), aiomqtt discussions (best articulation of router trade-offs).

---

## 6. Follow-up: code-level findings and design sketches

*Added after reviewing our own dispatch code and the rumqttc 0.25.1 source against the
survey's conclusions.*

### 6.1 Our slow-consumer policy: exists, but implicit (subscription_manager.rs)

Correction to the survey's framing: we do NOT lack a slow-consumer policy. The current
contract in `core/src/routing/subscription_manager.rs` is:

- each subscriber gets a bounded `mpsc::channel(500)` (`handle_subscribe`);
- dispatch uses `try_send`; on `Full`, a "slow send" task is spawned with
  `send_timeout(msg, 2s)`; on timeout the message is dropped with an `error!` log;
- at most 100 concurrent slow-send tasks — beyond that, immediate drop;
- on `Closed`, the subscriber is auto-unsubscribed.

So the policy is: *buffer 500 → 2s grace → drop*. What actually needs fixing:

1. **Unconfigurable** — 500 / 100 / 2s are hardcoded; `SubscriptionConfig` only carries
   `qos`. These belong in per-subscription config.
2. **Undocumented** — the drop behavior is invisible in public docs.
3. **Invisible to the application** — a drop is only a tracing line; no counter, no
   callback, no error surfaced to the subscriber. At minimum expose a per-subscriber
   dropped-message counter or a lag/overflow event (cf. HiveMQ's QoS-tiered policy).
4. **QoS-blind** — a QoS 1 message the broker considers delivered (we auto-ack today)
   can be silently dropped client-side, breaking the end-to-end at-least-once story.
   Ties directly into the manual-acks design (Tier 1.3 / §6.2).
5. **Ordering bug in the slow path** — while message A waits in a slow-send task, the
   actor keeps dispatching; if the consumer drains the channel, message B passes
   `try_send` and is delivered *before* A. Per-subscriber ordering is violated exactly
   when the subscriber is struggling. Fix (route subsequent messages for that subscriber
   through the same pending queue while a slow-send is in flight) or document it.

### 6.2 rumqttc 0.25.1 ack surface — verified against source

- **Inbound acks (backpressure primitive): already available, no fork needed.**
  `MqttOptions::set_manual_acks(true)` + `AsyncClient::ack(&publish)` exist in both the
  v3 API (`src/client.rs`) and the v5 API (`src/v5/client.rs`). "Don't PUBACK until the
  typed handler has processed" is implementable today.
- **Outbound confirmations (our publish → PUBACK, our subscribe → SUBACK): not exposed
  directly** — `publish()` returns `Result<(), ClientError>` with no packet id.
  ~~A FIFO correlation layer gives publish/subscribe futures without forking.~~
  **CORRECTED 2026-07-08 — the FIFO claim is refuted by a source audit of 0.25.1**:
  pkid collisions emit `Outgoing::AwaitAck` instead of `Publish` (with the real
  `Publish` event injected later from incoming-ack handling), reconnect retransmission
  re-emits events matching no call, and subscribe pkids can be reused while inflight
  (unobservable ambiguity). A pkid-aware wrapper remains possible for publishes but
  must shadow rumqttc's session state; subscribe correlation is unsound under load.
  Full analysis and failure scenarios with source citations:
  [FUTURE_WORK_RESEARCH.md §2](../FUTURE_WORK_RESEARCH.md).
- **Ranked plan (updated 2026-07-08):** (1) surface `SubAck.return_codes` now —
  fork-free; (2) **thin, API-additive fork of rumqttc** for real ack correlation
  (decided; sketch in FUTURE_WORK_RESEARCH.md §2), published under its own crate name
  since `[patch.crates-io]` doesn't propagate to downstream users; (3) upstream PRs for
  the bugs the audit found, opportunistically the notice mechanism; (4) long-term, keep
  the backend abstraction thin enough that a sans-IO core (`mqtt-protocol-core` style)
  stays an option — it doubles as the no_std path.

### 6.3 v3/v5 unification sketch

*(2026-07-08: this sketch was validated and refined by dedicated research —
see [DUAL_PROTOCOL_API_DESIGN_2026.md](./DUAL_PROTOCOL_API_DESIGN_2026.md)
for the decision, the ecosystem/Rust-precedent evidence, and the
locked-in-0.3 vs deferred-to-0.4 list.)*

One API surface designed in v5 terms, v3 as the degenerate case (paho v2's lesson
applied *before* the break instead of after):

- Unified `MessageMeta`, `PublishOptions`, reason-code types with `Option`-al
  v5-specific fields (properties, expiry); empty/`None` on v3.
- Internally an **enum backend** — `enum Backend { V3(rumqttc::AsyncClient),
  V5(rumqttc::v5::AsyncClient) }` behind a small internal trait — *not* a third public
  generic parameter (`MqttClient<F>` is already one generic too visible; a `<Protocol>`
  parameter would infect every signature). Protocol selected via config/URL
  (`mqtt://...?protocol=5`); one `match` per operation is noise next to network I/O.
- v5-only operations on a v3 connection return an explicit capability error — or are
  emulated where proven (MQTTnet emulates RPC on v3.1.1 via topic conventions).
- The macro and router are protocol-agnostic and need no changes.
- Alternative considered: a `client.v5()` namespace (Swift mqtt-nio) — good for exposing
  raw v5 extras later, but the unified surface is the right default at our altitude.

### 6.4 Cross-language typed wrappers — strategy

Generate wrappers for other languages **via AsyncAPI as the intermediate
representation**, not by emitting per-language code from the proc-macro directly:
the macro exports an AsyncAPI document (topics + parameters + JSON-Schema payloads);
existing generators (e.g. Modelina) produce payload models; we add only a thin
topic-layer template per language. First target: **TypeScript** — template literal
types can derive `{ location: string; device_id: string }` from
`"sensors/{location}/{device_id}/data"` *at the type level* with no codegen, and zod
supplies runtime validation + typed coercion (`z.coerce.number()`) that the TS type
system can't do for topic params. The niche is verified empty on npm. Constraints:
only cross-language serialization formats qualify (JSON/MessagePack/CBOR — another
argument against bincode-by-default), and exported param types must be restricted to a
portable subset (strings, numbers, UUIDs; custom enums via schema).
