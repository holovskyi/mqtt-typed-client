# `mqtt5` Crate (LabOverWire/mqtt-lib) — Backend Evaluation

*2026-07-08. Source-level evaluation of https://github.com/LabOverWire/mqtt-lib
(crate `mqtt5`, v0.35.1) as a potential low-level backend for mqtt-typed-client,
against three options: (a) migrate off rumqttc, (b) dual switchable backend,
(c) stay on rumqttc. File paths cited are relative to the mqtt-lib repo.*

**Verdict: stay on rumqttc for now; keep mqtt5 on the radar as an experimental
opt-in backend for 0.4+ if it stays maintained and splits client/broker. Do not
migrate outright; do not build the full dual-backend abstraction yet.**

## Project health

- **Bus factor = 1**: 225/230 commits by one author (Fabrício Bracht;
  "LabOverWire" is his Canadian micro-brand). ~1 year old (public since
  2025-11). Commit cadence decelerating: 73/mo at start → 2/mo by 2026-07,
  though v0.35.1 shipped 2026-07-06.
- 35 releases in a year = unstable API (breaking changes as recent as 0.34,
  0.35). Issues answered in days, but most are the author's own work log.
- **Adoption gap: ~2k downloads/month vs rumqttc's ~593k** (300×). Essentially
  untested by third parties.
- Engineering quality is unusually high for a solo project: **0 `unsafe`**,
  773 tests, 81 integration test files (real broker, TLS, QUIC, reconnect,
  proptest, turmoil deterministic networking), mutation-testing artifacts, and
  an **OASIS conformance suite** (183 tests tracked against 247 normative spec
  statements per `[MQTT-x.x.x-y]` id — `crates/mqtt5-conformance/`). Two
  academic papers in-repo (MQTT-over-QUIC). Test story is stronger than
  rumqttc's.
- License MIT OR Apache-2.0 — compatible. MSRV 1.88 (high; would drag ours up).
- **Client and broker in one crate — FIXED 2026-07-09.** Was: `lib.rs` had
  unconditional `pub mod broker;`, so every consumer compiled argon2, hyper(+
  rustls, JWKS), regex, toml, serde_json, flume, parking_lot; `quinn` (QUIC)
  and `tokio-tungstenite` on by default. We filed issue #100; author (fabracht)
  responded same day with PR #101 gating the broker behind a `broker` cargo
  feature (default-on, so existing consumers unaffected). Ships as `mqtt5`
  0.36.0. We verified branch `gate-broker-feature` against a client-only build
  (`default-features = false`): `cargo tree` drops **158 → 111 unique crates**,
  the whole broker subtree is gone (argon2, hyper, hyper-rustls, regex, toml no
  longer appear), and `cargo check` is clean. The <1-day turnaround is itself a
  positive maintenance signal. `mqtt5-protocol` is separately `no_std`-capable
  (relevant to our no_std direction); the client itself needs tokio full.
- TLS is rustls-only (ring). No native-tls option.

## Capabilities (source-verified)

- **Protocol**: v5 by default; v3.1.1 exists (`ProtocolVersion::V311`,
  protocol-level-4 encode/decode branches) but has **zero integration tests**
  and the conformance suite is v5-only — treat v3.1.1 as unproven. Adopting
  mqtt5 effectively means committing to MQTT 5.
- **Outbound acks — the headline win**: `publish().await` blocks until
  PUBACK/PUBCOMP via a oneshot keyed by packet id
  (`client/direct/mod.rs:565-705`, resolved by the reader task); broker-side
  failure surfaces as `MqttError::PublishFailed(ReasonCode)`; hardcoded 10s
  ack timeout. `subscribe()` returns the **granted QoS from the real SUBACK**
  (`client/mod.rs:437-448`). Exactly what rumqttc cannot do without a fork.
  Caveats: awaiting call, not an ack-future (no fire-and-forget-then-await;
  pipelining QoS 1 requires spawned tasks); success reason code not returned.
- **Inbound manual acks: absent.** PUBACK/PUBREC are sent automatically
  *before* the user callback runs (`client/direct/handlers.rs:70-150`).
  rumqttc's `set_manual_acks` wins here — and it is the primitive our
  backpressure/at-least-once-processing design needs.
- **Ownership model**: no pollable event loop, no raw publish stream —
  callback-per-subscription via `CallbackManager` (exact-match HashMap +
  linear-scan wildcards), dispatched through a single **unbounded** mpsc.
  Our router could adapt (per-filter callback forwards into our dispatch
  channel; wildcard filters supported; `on_connection_event` exposes
  `Connected { session_present }`), but we lose event-loop ownership,
  backpressure at the source, and our reconnect logic (theirs — auto-reconnect
  with backoff, session restore, resubscribe, outbound offline queue
  (`queue_on_disconnect`) — replaces it wholesale).
- **v5 coverage**: user properties, correlation data, response topic,
  subscription identifiers, retain handling, Receive Maximum flow control,
  topic aliases both directions, shared subscriptions ($share parsing),
  **enhanced auth (AUTH packet) with pluggable `AuthHandler` + shipped
  SCRAM-SHA-256 and JWT** — the area rumqttc lacks entirely.
- Transports: TCP, TLS, WebSocket/WSS, **QUIC** (`mqtt:// mqtts:// ws:// wss://
  tcp:// ssl:// quic://` — superset of ours).
- **No RPC utilities** in the client crate (request/response is only a wasm
  example) — corrects an earlier claim in CLIENT_LIBRARY_LANDSCAPE_2026.md.
- Performance: `Bytes` internally, but the public `Message` copies payload to
  `Vec<u8>` and topic to `String` — one alloc+copy per inbound message vs
  rumqttc's zero-copy `Bytes`.

## Migration mapping (our coupling points)

| Ours (rumqttc) | mqtt5 equivalent | Mismatch |
|---|---|---|
| `async_client.rs` owns `EventLoop::poll()`, routes publishes | per-filter callback → forward into our channel; `on_connection_event` | lose loop ownership, backpressure, our reconnect logic |
| `publisher.rs` `publish(topic, qos, retain, payload)` | `publish_with_options(...)` — resolves on ack, `PublishFailed(ReasonCode)` | payload `Bytes`→`Vec<u8>`; QoS 1 publish now awaits ~RTT (10s hardcoded timeout) |
| `config.rs` `MqttOptions`/`LastWill`/URL parsing | `ConnectOptions` builder + `WillMessage` (v5 will props); built-in URL parsing | straightforward; no `event_loop_capacity` analog |
| `error.rs` rumqttc error wrapping | flat `MqttError` (many stringified variants) | coarser network errors; gains `PublishFailed(ReasonCode)` |
| `subscription_manager.rs` subscribe/QoS | `subscribe_with_options` → granted QoS; own trivial `QoS` enum | their CallbackManager duplicates part of our trie (double routing) |

Dual-backend note: an internal trait is buildable
(`publish -> PublishAck::{Enqueued|Acked}`, `subscribe -> GrantedQoS`,
incoming-publish stream, connection-event stream), but it must degrade over
three real disagreements — rumqttc can't confirm acks/granted QoS, reconnect
ownership differs (ours vs theirs), error structures are incompatible. Cost:
two reconnect stories, two CI broker matrices, a semantics asterisk on every
ack-related doc line.

## Reasoning for the verdict

- Gain is real but narrower than hoped: outbound ack correlation, granted QoS,
  enhanced auth, QUIC, active author — but no inbound manual acks, no
  ack-futures, no RPC, v3.1.1 unproven.
- Risk is high: bus factor 1 on a cooling commit curve, 300× adoption gap,
  fast-moving 0.x API, forced broker deps, MSRV 1.88. Pinning our published
  0.x library to another solo 0.x library compounds churn.
- Cheap probe: file an issue asking for client/broker feature separation — the
  author responds to external issues within days; the response itself is a
  maintenance signal. DONE 2026-07-09: issue #100 → PR #101 same day, broker
  gated, client-only tree verified (158 → 111 crates). Probe passed; forced
  broker deps are no longer a blocker as of 0.36.0.
- Re-evaluate in 6–12 months: still maintained + client/broker split + some
  adoption → a v5-only `mqtt5` backend behind a cargo feature becomes a
  credible 0.4+ option. For ack semantics on our current stack, the decided
  path remains the thin rumqttc fork (FUTURE_WORK_RESEARCH.md §2).
