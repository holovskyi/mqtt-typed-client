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
  ceiling for v5 metadata: `IncomingMessage` and the macro-generated
  `FromMqttMessage::from_mqtt_message`.
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
  `IncomingMessage`/`FromMqttMessage` — always populated (simpler, heavier API)
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

### Recommendation

1. **Don't bank on an upstream PR** as a path on any timeline — 2+ years, 4 PRs,
   an RFC, zero in a release.
2. **First ask whether we actually need per-request correlation.** For most typed
   pub/sub, rumqttc's QoS guarantees suffice without explicit pkid feedback. The
   cheap first win is to stop discarding `SubAck.return_codes` and at least
   surface a broker-side subscription rejection.
3. **If correlation is a hard requirement:**
   - *Wrapper on our side* is possible (we already own the eventloop for
     routing) by catching `Outgoing::Publish/Subscribe(pkid)` + later
     PubAck/SubAck — but matching the first `Outgoing::*` to our call still needs
     serialization/ordering assumptions (hacky, only OK at low concurrency).
   - *Thin fork of rumqttc* is the cleanest pragmatic route: port the ready
     token/oneshot mechanism from the `ack-notify` branch / PR #916 / #1049 on
     top of 0.25.1 (what #1029's author did).
   - *Upstream PR* only opportunistically — don't plan around its merge.

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
