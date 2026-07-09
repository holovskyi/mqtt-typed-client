# Future Work — Research Notes

*Deep-dive findings backing the [ROADMAP](./ROADMAP.md). Captures the concrete
state of `rumqttc` and the surrounding ecosystem as of **2026-07-01**, so that
when we pick up these items we don't re-investigate from scratch. Versions and
issue/PR numbers are point-in-time — re-verify before acting.*

Three themes are covered:

1. [MQTT v5 support](#1-mqtt-v5-support)
2. [Publish / subscribe acknowledgment correlation](#2-publish--subscribe-acknowledgment-correlation)
3. [`no_std` / embedded variant](#3-no_std--embedded-variant)

Baseline: the workspace pins `rumqttc = "0.25.1"` (`Cargo.toml`, root) with
`default-features = false`; `0.25.1` (released 2025-11-21) is already the latest
published version, so there is nothing to upgrade to. The `[Unreleased]` section
of rumqttc's changelog is empty.

*Naming note: "v4"/"v5" throughout follows rumqttc's module naming, which uses the
protocol-level byte from the CONNECT packet — MQTT 3.1 = level 3, MQTT 3.1.1 =
level 4, MQTT 5.0 = level 5. So rumqttc's default "v4" API is MQTT 3.1.1.*

---

## 1. MQTT v5 support

### Scope depends entirely on the goal

`rumqttc` does **not** unify v4 and v5 — they are two parallel type universes
(`rumqttc::v5::{AsyncClient, EventLoop, MqttOptions, QoS, Publish, ...}` vs the
crate-root v4 types). That single fact drives the cost:

| Variant | Effort | What it means |
|---|---|---|
| **A. Straight v4 → v5 swap** (no new features in public API) | **M** | Mechanical but cross-cutting retyping across ~8 files. Low risk, no features gained. |
| **B. Full v5** (user properties, correlation/request-response, reason codes, subscription identifiers in the typed API + macros) | **L–XL** | The message type is currently `(Arc<TopicMatch>, Result<T>)` with no slot for metadata, and `publish()` takes nothing but `data`. Exposing v5 metadata changes the public contract and the macro-generated `FromMqttMessage`. |
| **C. Dual v4 + v5** (both protocols at once) | **XL** | rumqttc shares no trait across v4/v5, so we must abstract the whole client layer behind a trait or duplicate the module. |

The core difficulty for B/C: there is nowhere to *put* v5 metadata today. Adding
it is a public-API + macro change, not a local patch. Public fields also leak
rumqttc types (`MqttClientConfig.connection: MqttOptions`,
`SubscriptionConfig.qos: rumqttc::QoS`), so any v5 work is a breaking change →
schedule for a `0.3.x` major.

Good news: the engine already has a **protocol-neutral `QoS`**
(`mqtt-topic-engine/src/qos.rs`); its conversions are just v4-bound and need
parallel v5 variants.

### v5 maturity in rumqttc 0.25.1

**`v5` is not gated** — `pub mod v5;` in `lib.rs` has no `#[cfg(feature=...)]`,
and there is no `v5`/`mqtt5` cargo feature. It compiles unconditionally and is
fully compatible with the flags we use (`url`, `use-rustls`, `websocket`,
`use-native-tls` — all transport/URL concerns, unrelated to protocol version).
`v5::MqttOptions::parse_url` works under the same `url` feature.

Feature status (verified against the published 0.25.1 sources):

| Works ✅ | Problematic |
|---|---|
| user properties (`*_with_properties` API), reason codes, subscription identifiers, topic aliases (both directions), message/session expiry, request/response (response topic + correlation data), content type, retain-handling, shared subscriptions (via `$share/`), max packet size | **Enhanced Auth (AUTH packet) — effectively unsupported** ❌: packet type 15 isn't even decoded (`PacketType` has no `Auth` variant), can't be sent, re-auth flow isn't driven. Only a stub struct. |
| | **Flow control (Receive Maximum) — partial** ⚠️: broker's limit on our outbound publishes is honored, but there's no inbound backpressure of our own. |

**Verdict:** for ~95% of typed pub/sub scenarios (properties,
correlation/response, expiry, aliases) the v5 base is ready to build on. But the
v5 API is self-described as not finalized (`// TODO: Should all the options be
exposed as public?`), so the typed layer should isolate those unstable spots.
Enhanced auth cannot be offered without patching rumqttc.

### Files the swap touches

Protocol coupling is concentrated but cross-cutting:

- `Cargo.toml` (root) + `core/Cargo.toml` — rumqttc features.
- `core/src/client/async_client.rs` — most coupling: `ConnAck` matching, the
  eventloop `run()` match arms, `p.topic` (which is `Bytes` in v5, not `String`).
- `core/src/client/publisher.rs` — `publish(topic, qos, retain, payload)` gains
  a `properties` argument in v5; imports `rumqttc::QoS`.
- `core/src/client/config.rs` — `MqttOptions`, `LastWill`, `OptionError`; the
  public `connection: MqttOptions` field.
- `core/src/client/last_will.rs` — `TypedLastWill<T>`, optional
  `LastWillProperties`.
- `core/src/client/error.rs` — all `From` conversions for rumqttc error types.
- `core/src/routing/subscription_manager.rs` — `SubscriptionConfig.qos` (public),
  `subscribe`/`unsubscribe` calls, `String`-typed topic dispatch.
- `core/src/client/subscriber.rs` + `core/src/structured/subscriber.rs` — the
  ceiling for v5 metadata: `SubscriberEvent` (the `ReceiveEvent` alias) and the
  macro-generated `FromMqttMessage::from_mqtt_message`.
- `mqtt-topic-engine/src/qos.rs` — v4-bound conversions; add v5 parallels.
- `macros/src/codegen_typed_client.rs` — generated `publish()`/subscriber code;
  the `#[mqtt_topic(...)]` attribute syntax may need to grow if metadata is
  exposed.

### Open questions before committing

- Which is the target: A (swap), B (full typed v5), or C (dual)? B/C need an
  architectural decision — generic `MqttBackend` trait vs a separate `client::v5`
  module vs a `mqtt-v5` feature flag.
- Where do message metadata live? A `MessageMeta { user_properties,
  correlation_data, response_topic, content_type, ... }` slot on
  `SubscriberEvent`/`FromMqttMessage` — always populated (simpler, heavier API)
  vs only when the type asks for it.
- Reconnect/resubscribe logic is v4 `session_present`-based; v5 session-expiry
  semantics differ — re-check.

---

## 2. Publish / subscribe acknowledgment correlation

*(Overlaps with the two ack items already in ROADMAP under "Core Protocol
Enhancements".)*

### The technical constraint is real (confirmed from source)

Per-request ACK correlation is **not possible** with released rumqttc:

- Both v4 and v5 `publish`/`subscribe` (and `try_*`, `_with_properties`) return
  `Result<(), ClientError>` — **no pkid**.
- **The eventloop assigns the pkid, not the client**: in
  `state.rs::outgoing_publish`, `publish.pkid = self.next_pkid()` runs when the
  eventloop dequeues the request — *after* the client call already returned
  `Ok(())`. The caller physically cannot know its own pkid at the call site.
- `Event::Outgoing(Outgoing::Publish(pkid))` carries **only the pkid, no
  topic/payload** — with several in-flight publishes there's no way to match it
  back to a specific call (the race). No `NoticeFuture`, no callback, no
  `publish_with_pkid`.

So `Ok(())` means "queued to the eventloop", not "broker acknowledged".
Currently the code discards this entirely: `async_client.rs` `run()` handles only
`ConnAck`, `Publish`, `Disconnect`; `SubAck`/`PubAck`/`PubComp` fall into a
catch-all `debug!` log. In particular `SubAck.return_codes` (where the broker can
*reject* a subscription) are silently dropped.

QoS nuance: SUBACK always exists; PUBACK only for QoS 1; PUBREC/PUBCOMP for QoS
2; QoS 0 has no acknowledgment at all. The project defaults to QoS 1
(`publisher.rs`, `subscription_manager.rs`).

### Upstream is a dead end for this

This is a long-standing, unresolved upstream ask:

| # | What | Status (2026-07-01) |
|---|---|---|
| #349 | "Get packet id for publish" | open ~4.5 years |
| #805 | RFC: publish/subscribe return a promise resolving to pkid | open, 30 comments |
| #851, #916, #925, #1049 | four separate PRs for this feature (NoticeFuture, token mechanism) | all open 2+ years |
| #921 + #946 | token mechanism actually *finished*… | …merged into the abandoned `ack-notify` branch (dead since 2025-02); **in no release** |

rumqttc itself is in **slow maintenance**: last `main` commit ~2026-05-01
(~2 months quiet); yearly releases (0.24 → 0.25.0 → 0.25.1); 76+ open PRs, oldest
from 2022; external feature PRs systematically stall (mostly dependabot + the
maintainer's own PRs merge). Issue #1029 ("I Will Be Maintaining a Fork For
Rumqttc") is a contributor publicly forking because PRs sit for years.

### Source verification of the fork-free wrapper idea (2026-07-08)

An adversarial source audit of the published 0.25.1 crate settled the dispute
between "a FIFO correlation layer works without a fork" (an earlier claim in
CLIENT_LIBRARY_LANDSCAPE_2026.md §6.2) and "only a fork is sound". **The naive
FIFO mechanism is refuted**; a much heavier pkid-aware wrapper is possible for
publishes but fragile; subscribe correlation has an unfixable-from-outside gap.

What holds: requests flow through a bounded flume MPSC (`eventloop.rs:103`),
are dequeued one at a time, and `handle_outgoing_packet` emits exactly one
`Outgoing` event per request synchronously, with pkid assigned in
`outgoing_publish`/`outgoing_subscribe` (`state.rs:311-348, 409-429`). In-stream
ordering is deterministic.

What breaks naive FIFO matching:

1. **pkid collision → `Outgoing::AwaitAck(pkid)`, not `Publish`**
   (`state.rs:318-329`). When `outgoing_pub[pkid]` is still awaiting an ack
   (pkid rollover + out-of-order broker acks), the publish is stashed in
   `state.collision` and the real `Outgoing::Publish(pkid)` is injected *later,
   from inside incoming-ack handling* (`state.rs:236-245, 292-298`). A pure FIFO
   matcher pops the wrong entry.
2. **Reconnect retransmission**: unacked QoS 1/2 publishes move to
   `EventLoop::pending` (`eventloop.rs:127-143`) and after reconnect re-emit
   `Outgoing::Publish(pkid)` events matching no new client call.
3. **`session_present == false` after reconnect → `pending.clear()`**
   (`eventloop.rs:160-163`): those publishes vanish with no event ever.
4. **Subscribes are never retransmitted** (not stored in state,
   `state.rs:409-429`) — a subscribe that hit the wire before a disconnect gets
   no SubAck, ever.
5. **Subscribe pkid reuse — the residual, unfixable gap**: sub/unsub pkids come
   from the same `next_pkid` counter with *no collision check*
   (`state.rs:417, 435`; wraps at `max_inflight`, `state.rs:486-500`). With more
   than `max_inflight` unacked sub/unsub requests, two inflight subscribes can
   share a pkid — SubAck matching becomes ambiguous, and this is not observable
   from the event stream. Only avoidable by self-throttling.

A *sound* fork-free layer for publishes therefore must: serialize all calls
through itself, bind on `Publish(pkid)` OR `AwaitAck(pkid)`, ignore re-emissions
for already-mapped pkids, fail everything on `session_present == false`, and
fail inflight subscribes on every connection error — i.e. re-implement a shadow
of rumqttc's session state, bug-compatible with undocumented event-emission
details (which even differ between v4 and v5 in ConnAck-vs-stale-events
ordering: `eventloop.rs:170` vs `v5/eventloop.rs:162-163`). Buildable, fragile,
and still incomplete for subscribes.

Also verified: 0.25.1 and `main` (e886a78, 2026-05-01; `state.rs` byte-identical
to the release) contain **no** Notice/Token/ack-notification API whatsoever.
`manual_acks` for *inbound* acks exists and works in both v4 and v5
(`lib.rs:708`, `v5/mod.rs:521`, `client.rs:115-131`).

**rumqttc bugs found during the audit** (worth upstream issues, and fixing in a
fork): (a) `MqttState::clean()` never clears `state.collision`
(`state.rs:103-130`) — a collided publish is lost on reconnect AND keeps the
request branch disabled until `CollisionTimeout` (`state.rs:381-385`);
(b) the subscribe pkid-reuse above is also a spec violation (packet id reused
while still unacknowledged); (c) v4 rotates `pending` around `last_puback` on
`clean()` while v5 doesn't (`state.rs:103-130` vs `v5/state.rs:144-168`) —
inconsistent retransmission order.

### Recommendation (updated 2026-07-08 — decision: fork)

1. **Don't bank on an upstream PR** as a path on any timeline — 2+ years, 4 PRs,
   an RFC, zero in a release.
2. **Cheap first win stays valid**: stop discarding `SubAck.return_codes` and
   surface broker-side subscription rejection — needs no fork.
3. **For real correlation, a thin fork is the decided route** (the fork-free
   wrapper is refuted as unsound-in-general above). Minimal fork sketch:
   - carry an optional completion sender on the request —
     `Request::Publish(Publish, Option<oneshot::Sender<Result<Pkid, AckError>>>)`
     or a dedicated `NoticeTx`;
   - widen `MqttState::outgoing_pub: Vec<Option<Publish>>` (`state.rs:64`) to
     store the sender alongside the publish so it survives collision and
     retransmission;
   - resolve it in `handle_incoming_puback` (`state.rs:222`) /
     `handle_incoming_pubcomp` (`state.rs:284`); fail it in `clean()` and the
     `session_present == false` drop path;
   - a parallel `HashMap<pkid, NoticeTx>` for subscribes, resolved in
     `handle_incoming_suback` (`state.rs:187`), plus a collision check for
     subscribe pkids;
   - mirror in `src/v5/`. ~5 functions per protocol version + two enum/struct
     field changes. Can be made API-additive (new `publish_with_notice()`
     methods; new `Request` variant instead of changing the existing one) so the
     fork stays a drop-in superset of upstream.
   - Since this crate is published on crates.io, `[patch.crates-io]` does NOT
     propagate to downstream users — the fork must be *published under its own
     name* (what issue #1029's author did).
4. *Upstream PRs* (the bugs above; opportunistically the notice mechanism) —
   file them, don't plan around their merge.

### rumqttc-next — the fork from issue #1029 already did this (checked 2026-07-08)

The fork announced in bytebeamio/rumqtt#1029 is **rumqttc-next**
(https://github.com/thehouseisonfire/rumqtt, crates.io: `rumqttc-next` facade +
`rumqttc-v4-next` / `rumqttc-v5-next` / `rumqttc-core-next` / `mqttbytes-core-next`;
library target is still named `rumqttc`, so imports are unchanged). Verified
against a clone at commit 2026-07-07 (yes, active the day before this check):

- **The ack-notification feature we planned to fork for is implemented**:
  `publish_tracked()` / `subscribe_tracked()` / `unsubscribe_tracked()` return
  notices resolving to `PublishResult::{Qos0Flushed, Qos1(PubAck),
  Qos2Completed(PubComp)}` with explicit failure modes
  (`PublishNoticeError::{Recv, SessionReset, Qos0NotFlushed, ...}`,
  `SessionReset` covering the `session_present == false` drop path that naive
  wrappers can't see). Untracked `publish()` remains — API superset
  (`rumqttc-v4/src/notice.rs`, `client.rs:713-910`; mirrored in v5).
- **Our audit's bugs are addressed**: changelog explicitly lists "Prevent
  packet identifier reuse across publish, PUBREL, subscribe, and unsubscribe
  flows, returning state errors instead of silently colliding identifiers"
  (= our subscribe pkid-reuse bug); collision handling is reworked
  (`next_pkid()` now returns `Option`, no silent reuse); notices complete only
  after session-store durability barriers.
- Beyond our fork scope: manual acks with reason codes (`AckMode::{Automatic,
  Manual}`, PR #855), validated `Topic`/`TopicFilter` types (= our Tier 2.9
  idea), opt-in persistent `SessionStore`, `EventLoop::into_stream()`,
  per-requirement MQTT spec-compliance tracking (`docs/spec/*.requirements.json`),
  scheduler rework so control packets aren't stuck behind publish backpressure.
- Maintainer stance: "issues and PRs are very welcome" (2026-05-04 comment),
  responds within days, explicitly offered to help upstream the changes back
  to rumqttc when bytebeam is ready.
- **Risks**: bus factor 1 (pseudonymous), exists only since 2026-02, heavy
  breaking-change churn between releases (the Unreleased changelog section is
  full of Breaking Changes — e.g. the publish API was just unified around
  `PublishOptions`), repo carries `AGENTS.md` + `TODO1-5.md` suggesting heavy
  AI-agent-assisted development (the spec-requirement tracking suggests rigor,
  but audit before trusting), adoption unknown/small.

**Update 2026-07-08, after a full adversarial audit: verdict is
adopt-with-mitigations — do not write our own fork.** All §2 failure scenarios
verified handled at fork HEAD in both v4 and v5; test suite is genuinely
stronger than upstream's; adoption is small but real (rustfs, 29.6k stars,
uses the tracked-notice API). Mandatory mitigations: pin the audited git rev
(crates.io 0.33.2 LACKS several audited fixes), diff-review every upgrade,
hide the fork fully behind our own public types. Full report incl. the
scenario table, risks, the maintainer's port of this library
(`mqtt-typed-client-next`), and the validated migration recipe:
[research/RUMQTTC_NEXT_AUDIT_2026.md](./research/RUMQTTC_NEXT_AUDIT_2026.md).

Key source anchors: `rumqttc/src/client.rs`, `rumqttc/src/v5/client.rs`,
`rumqttc/src/state.rs` (`outgoing_publish` / `next_pkid`), `rumqttc/src/lib.rs`
(`enum Outgoing`); our side: `core/src/client/async_client.rs` (eventloop),
`core/src/client/publisher.rs`, `core/src/routing/subscription_manager.rs`.

---

## 3. `no_std` / embedded variant

### Feasible, but only as a separate crate on a different stack

A full `no_std` port of `core`/root is **not realistic** — `tokio` (full) and
`rumqttc` hard-require `std` (networking via `TcpStream`, async runtime). This is
architectural, not a feature-flag issue.

The realistic niche is **`no_std + alloc`** (embassy-net + heap), *not*
bare-metal without a heap: the project's whole value (trie, `String` topic
parameters, serde-json/cbor) is naturally alloc-based.

### Recommended stack

```
[ #[mqtt_topic] macro + mqtt-topic-engine trie + serde ]   ← reuse
[ rust-mqtt (v5, async, alloc feature) ]                   ← replaces rumqttc
[ embedded-io-async transport ]
[ embassy-net TcpSocket ] + [ embassy-executor / embassy-time ]  ← replaces tokio
```

**Why rust-mqtt** as the primary candidate: its `async fn poll() -> Event` /
`subscribe` / `publish` model is closest to our current async API; it's
**std + no_std** (so development and tests can start on desktop TCP); alloc is
optional so the engine and serde stay alloc-based. Alternatives: `minimq`
(no-alloc, v5, more mature, but a poll model that costs more to adapt);
`mqttrust` (v3.1.1 + v5, but ships its own topic matcher that duplicates the
engine, plus opinionated reconnect/keepalive).

### Reuse vs rewrite

**Reuse (the core value):**
- `mqtt-topic-engine` — 100% of the logic; mechanical port to `no_std + alloc`
  (`std::HashMap/HashSet` → `hashbrown`; `std::sync::{Arc, Mutex}` →
  `alloc::sync::Arc` + `critical-section`/`spin` Mutex; the rest is in
  `core`/`alloc`). Dependencies are nearly ready: `arcstr`, `smallvec`,
  `thiserror` 2.0, `tracing` all have no_std modes; `lru` (behind the cache
  feature) needs checking. This is the biggest "free" win and is worth doing
  independently — the engine is already published standalone.
- Typed conversion layer (`core/src/structured/subscriber.rs`:
  `FromMqttMessage`, parameter extraction) — pure logic, no tokio coupling.
- Macro pattern analysis (`macros/src/analysis.rs`, `naming.rs`) — proc-macro
  runs on the host, std-agnostic. Only the codegen template needs rework.
- serde layer — `serde` core is no_std; prefer `postcard` (no_std-native) as the
  embedded default; `serde_json`/`ciborium`/`bincode 2.0` work with alloc.

**Rewrite (the tokio-bound runtime/actor layer):**
- `core/src/routing/subscription_manager.rs` — `tokio::spawn` + `mpsc` +
  `select!` + `FuturesUnordered` fan-out. This is the heart of the tokio binding
  and does **not** port; becomes a single `recv/poll` loop + `engine.matches()` +
  dispatch (optionally fan-out via `embassy-sync`).
- `core/src/client/async_client.rs`, `core/src/connection.rs` — rebuild on
  embassy-executor tasks + `embassy-time` timeouts.
- The codegen template in `macros/src/codegen_typed_client.rs`.

### Effort estimate (1 experienced Rust dev)

| | Estimate |
|---|---|
| Shared base: engine → no_std+alloc, typed layer, macro codegen, std tests | ~2 weeks |
| **Variant A (rust-mqtt, recommended)** total | **~4–5 weeks** |
| Variant B (minimq, no-alloc) total | ~6–8 weeks (riskier: no-alloc client vs alloc-based engine conflict) |

### Cheapest first step (~1.5–2 weeks) — retires the main unknown before touching hardware

1. Port `mqtt-topic-engine` to `no_std + alloc` behind a feature flag (`std`
   default on) — useful independently.
2. Build a minimal PoC: rust-mqtt in **std mode** (plain desktop TCP) + engine
   routing + one typed subscribe/publish, **no embassy, no hardware**. This
   proves the typed layer lives on a different client and that rust-mqtt's API
   lets us splice in trie routing.

Only if the PoC succeeds: swap transport to embassy-net and build a real
MCU/QEMU example as a separate crate (`mqtt-typed-client-embedded`). Keep minimq
(Variant B) as a second-tier option for strictly memory-constrained targets.

### Main risks

- alloc vs no-alloc split — true bare-metal (minimq-style) means rewriting the
  engine on static buffers, which is a different project. `no_std + alloc` is the
  pragmatic target.
- Fan-out model — the current actor fans one message to many subscribers via
  tokio channels; embedded is typically single-consumer. Decide the product
  scenario (single-consumer vs embassy-sync multi-subscriber).
- Both rust-mqtt and minimq are **v5-only** (rust-mqtt's v3.1.1 is planned only);
  v3.1.1 on embedded means mqttrust, which duplicates the topic matcher.
- The macro generates against concrete `core` types; if embedded-core has a
  different API, the codegen branch diverges and two targets cost more to
  maintain.
