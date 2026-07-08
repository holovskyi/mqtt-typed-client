# Dual-Protocol (MQTT 3.1.1 / 5.0) Public API Design

*2026-07-08. Research into how our public API should support both protocols,
with an ecosystem survey, Rust-precedent analysis, and a cost sketch in our
codebase. Refines CLIENT_LIBRARY_LANDSCAPE_2026.md §6.3. Assumes the backend
is rumqttc-v4-next / rumqttc-v5-next (two separate crates sharing an API
shape — see RUMQTTC_NEXT_AUDIT_2026.md).*

**DECISION: Design A — one unified public surface; protocol is a runtime
value chosen at connect (config / `?protocol=` URL); internally an enum over
the two backend crates; per-backend cargo features are additive (both on by
default) and only strip compiled-in capability, never change types.
Optionally add a `client.v5()` accessor namespace (B2) for raw v5 extras
in 0.4. No protocol type parameter, no split public modules.**

## Candidates evaluated

- **A. Unified surface + internal runtime enum** (chosen).
- **B. Fully split API** (`client::v4::MqttClient` / `client::v5::MqttClient`;
  B2 = unified + `client.v5()` accessor). Rejected as primary: doubles the
  typed layer — the macro would emit impls for two client types (collapses
  into D internally) while duplicating the public surface. Documented cost on
  the split side: rumqttc's re-convergence PR #861 open for years. B2 survives
  as an additive 0.4 option.
- **C. Mutually exclusive cargo features** (what the rumqttc-next port of our
  lib does). **Disqualifying for a library**: Cargo book requires features to
  be additive; the diamond case (dep A wants `v4`, dep B wants `v5`) either
  hard-fails the end user's build or silently picks one backend. Real-world
  breakage: sqlx #950 (intermittent workspace build failures → fixed in 0.7
  by additive features + runtime dispatch), rustls #1877 (ring + aws-lc-rs via
  two deps → runtime panics → fixed by runtime `CryptoProvider`). Acceptable
  only for leaf crates (embassy HALs, his port); not for us.
- **D. Typestate `MqttClient<F, P: Protocol = V5>`**. Rejected: infects ~9
  public types (publisher, subscribers, builder, config, connection, manager)
  plus a sealed trait with ~6 associated types, plus every macro-generated
  struct in `macros/src/codegen_typed_client.rs`. And the default type param
  fails at the primary UX site: defaults apply in type position only — in
  expression position (`MqttClient::<Bincode>::connect(url)`) the default
  never participates in inference (RFC 213 fallback never stabilized,
  rust-lang/rust#27336) → E0282 or full turbofish. Also makes URL-based
  protocol selection impossible (protocol would be a type, decided before the
  URL is parsed). Type-gates a handful of knobs at the price of every
  signature in the crate.

## Why the ecosystem's "split v3/v5 APIs" pattern does not transfer to us

The genuine wire-divergence kernel is real: v5 acks carry reason codes +
properties (return types genuinely differ); session semantics differ
(`clean_session` vs `clean_start` + expiry — auto-resubscribe logic that is
right on v4 is wrong on v5); a typed split makes "user properties on a v3
socket" unrepresentable.

But the splits we observe are mostly retrofit economics, not from-scratch
design: rumqttc's v5 module was born as a copy-paste (PR #351: "copied from
mqttbytes… v4 not changed") and drifted; paho.golang is a "deliberately
incompatible" rewrite in a separate repo; paho.mqtt.c grew `*5` function
suffixes its own author is "not particularly happy" with. **The decisive
evidence is HiveMQ — the one team that designed dual-protocol from scratch:
split only thin public `*View` facades over ONE v5-shaped engine with v3 as
the degenerate case.** Even the strongest split advocate unifies the
machinery. Our position is more favorable still: the split-where-real already
exists *below* us (the two -next backend crates), and our surface is
topic-typed pub/sub where v4 is a semantic subset of v5. The divergences land
in exactly two places we control: the ack-notice result type (v5-shaped,
`Success` on v4) and our options facade (v5-shaped, `session_expiry` with a
documented v4 mapping).

## Ecosystem survey (condensed)

| Library | Pattern | Retrospective |
|---|---|---|
| rumqttc | split `v5` module (copy-paste origin, PR #351) | API drift; re-convergence PR #861 open for years |
| paho.mqtt.c | one handle + `*5` suffixes | author unhappy with the naming |
| paho.mqtt.python | runtime flag, **protocol leaked into callback signatures** | v2.0 `CallbackAPIVersion` ecosystem breakage (aiomqtt #293 etc.) |
| MQTT.js | runtime `protocolVersion`, one API | v5 properties **silently ignored** on v3; version-conditional bugs (#1073, #1157) |
| HiveMQ Java | split thin views over one v5-shaped engine | stated design; no regrets found |
| emqtt | runtime `{proto_ver, v5}`, default v5 | — |
| mqtt-nio (Swift) | one client + `client.v5` accessor, throws `versionMismatch` once at accessor | maintained; no regrets found |
| MQTTnet | runtime enum, one API | oscillated: throw on v5-props-on-v3 (#848) → silently ignore (#885) — complaints both times |
| paho Go | whole-repo split (v3 lib vs v5 rewrite) | the split is the retrospective |

Pattern: dynamic languages use a runtime flag; stronger type systems split
harder. **Both documented regret cases are sloppy-unified-surface failures
(callback-shape leakage; silent drop vs throw churn) — fixed by our two
hardening rules, not by splitting.**

## Hardening rules for Design A

1. **Never silently drop v5 data on a v4 connection.** Explicit typed
   capability error, validated as early as possible (config/connect-time
   where feasible; rustls lesson).
2. **Protocol never changes the shape of receive-side signatures.**
   `MessageMeta { qos, retain, dup, v5: Option<Mqtt5Meta> }` — one stable
   shape, `None` on v4. **Never cfg-gate the `v5` field**: a feature that
   changes a public struct's shape is non-additive by definition. Additive
   features may strip *backends*, never *types*.

## Cost sketch in our codebase (A, incremental in 0.4 once the 0.3 facade exists)

- `config.rs`: protocol-neutral fields + `ProtocolVersion`, convert to the
  chosen backend's options at connect; URL grammar gains `?protocol=`.
- `async_client.rs` (heaviest): `enum BackendClient { V4(..), V5(..) }`
  (~5 methods × 2 near-identical arms — the -next crates share an API shape,
  a small private macro can stamp them); two event loops normalizing into one
  private `BackendEvent` enum (~150 lines; the v5 arm populates `Mqtt5Meta`).
- `publisher.rs`, `subscription_manager.rs`, `connection.rs`,
  `subscription_builder.rs`: swap the client field for `BackendClient`.
- `error.rs`: `From` impls for both backend crates into our own error enum;
  our own reason-code enum replaces the leaked `rumqttc::ConnectReturnCode`.
- `last_will.rs`: optional v5 will properties. `mqtt-topic-engine/src/qos.rs`:
  v5 conversions parallel to v4.
- **Untouched: `macros/` entirely, the trie router, `FromMqttMessage`
  signatures. Public generic parameters infected: 0.** Same file list as
  FUTURE_WORK_RESEARCH.md §1's "files the swap touches" — no extra surface.

## Locked in 0.3 vs deferred to 0.4

**Lock in 0.3 (breaking if done later):**
1. Own facade types for everything backend-flavored (`MqttOptions`, `QoS`,
   `Transport`, errors) incl. a reason-code enum designed as the **v5
   superset** (v4 codes map in); kill the `rumqttc::ConnectReturnCode` leak
   in `ConnectionEstablishmentError::BrokerRejected` (core/src/client/error.rs).
2. `MessageMeta` with `v5: Option<Mqtt5Meta>` present (always `None` in 0.3);
   `Mqtt5Meta` defined `#[non_exhaustive]` now.
3. `PublishOptions` as a `#[non_exhaustive]`/private-field builder.
4. All public error enums `#[non_exhaustive]` (capability-error variant lands
   in 0.4 without a major).
5. `ProtocolVersion` (`#[non_exhaustive]`, `V4` default) in config +
   `?protocol=` URL grammar — parsed in 0.3, `5` → clean "arrives in 0.4"
   error.
6. The negative decision: no protocol type parameter, no split public modules.
7. Options facade v5-shaped: `session_expiry`-style config with documented v4
   mapping, not `clean_session: bool` verbatim.

**Defer to 0.4:** the `BackendClient` enum and v5 event loop (0.3 ships one
backend behind the facade); `Mqtt5Meta` population; `PublishOptions` v5 knobs;
capability-errors-vs-`client.v5()` accessor spelling; typed RPC and shared
subscriptions (incl. whether RPC gets v4 emulation via topic conventions —
MQTTnet proves it works); additive `backend-*` size-stripping features;
protocol negotiation/fallback (no evidence anyone needs it — don't design
for it).

Key sources: Cargo book (Features), sqlx #950/#1669, rustls #1877, rumqtt
PR #351/#861, MQTTnet #848/#885, aiomqtt #293, HiveMQ api-flavours docs,
mqtt-nio `MQTTConnection+v5.swift`, rust-lang/rust#27336.
