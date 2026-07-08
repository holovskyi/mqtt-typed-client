# rumqttc-next — Adoption Audit

*2026-07-08. Adversarial source audit of the rumqttc-next fork
(https://github.com/thehouseisonfire/rumqtt) at HEAD `f8af91b` (2026-07-07),
plus an analysis of the maintainer's port of this library
(`mqtt-typed-client-next`) and an adoption check. Complements
FUTURE_WORK_RESEARCH.md §2 (which documents why upstream rumqttc needs a fork
at all) and MQTT5_CRATE_EVALUATION_2026.md (the rejected alternative backend).*

**VERDICT: adopt-with-mitigations — do not write our own fork.**

## Why adopt

The state machine is upstream's battle-tested code evolved (~75% of upstream
`state.rs` lines survive verbatim), not a rewrite. The tracked-notice layer is
architecturally correct: oneshot senders threaded through `RequestEnvelope` →
`MqttState` slot vectors → resolved via a deferred-notice effects path that
even orders completion after session persistence. Every failure scenario that
makes ack correlation unsound on upstream (FUTURE_WORK_RESEARCH.md §2) was
verified handled at HEAD, in both v4 and v5, usually with a state-machine test
attached:

| Scenario | Status | Key evidence (rumqttc-v4/src) |
|---|---|---|
| pkid collision (upstream: publish lost on `clean()`, request branch blocked) | Handled | `next_pkid()` scans for a *free* pkid — auto-assigned collisions impossible (state.rs:1400-1421); stashed publish keeps its notice and replays even across reconnect (state.rs:1097-1098, 1343-1359); fails with `SessionReset` on session loss (state.rs:651-656) |
| Reconnect retransmission (session_present=true) | Handled | `clean_with_notices()` moves unacked publishes *with their notice senders* into pending (state.rs:481-554); replay re-registers the notice (state.rs:1114-1115); DUP set only if a flush was attempted; wire-level test asserts DUP=1 + oldest-first (v5 tests/reliability.rs:1199) |
| session_present=false (upstream: silent `pending.clear()`) | Handled | every pending/inflight notice fails with `SessionReset` (eventloop.rs:289-299, 478-534; state.rs:638-658); nothing vanishes silently |
| Sub/unsub pkid sharing; subscribes never retransmitted | Handled | control pkids checked against ALL in-use pkids (state.rs:1440-1457); pending sub/unsub requeued on reconnect with notices (state.rs:528-542); SUBACK count mismatch → protocol violation |
| v4/v5 retransmission-order divergence | Handled | v5 uses the same split-at-`last_puback` rotation (v5 state.rs:828-835) |
| EventLoop dropped mid-flight | Handled | oneshot drop → `PublishNoticeError::Recv`, never hangs (notice.rs:55-68) |
| QoS0 tracked semantics | Handled | `Qos0Flushed` only after actual socket flush (eventloop.rs:1049-1092) |
| Duplicate/unsolicited acks | Caveat | `StateError::Unsolicited` → connection teardown + replay; a misbehaving broker sending one duplicate PubAck causes a reconnect cycle |
| pkid exhaustion | Caveat | normal path is scheduler backpressure (state.rs:400-419); the defensive `InvalidState` drop path is reachable only with hand-rolled pkids |
| Rejected SubAck | Handled | `SubscribeNotice::wait()` returns raw SubAck; `wait_completion()` → typed `SubAckFailure` (notice.rs:397-408) |

Test quality (adversarial read, not counted): ~1063 tests vs upstream's 92;
notice/collision/reconnect/manual-ack/exhaustion tests are genuine
state-machine and wire-level tests, some citing MQTT spec IDs; no
asserts-the-implementation cases found. `docs/spec/*.requirements.json`
compliance tracking is real verification with honest "partial"/"unreviewed"
statuses (spot-checked). Manual acks strictly better than upstream
(unsolicited/duplicate/QoS-mismatch rejected).

## Mandatory mitigations

1. **Pin a git rev, not crates.io 0.33.2.** Several audited soundness fixes
   (incl. pkid-reuse prevention) and the current publish API are in
   `[Unreleased]`; the published 0.33.2 (2026-05-23) does NOT contain the
   audited behavior. Pin the audited rev or wait for the next release and
   re-verify. (Repo bookkeeping is sloppy: HEAD manifests say 0.33.0, the
   CHANGELOG is missing 0.33.2.)
2. **Treat every upgrade as a supply-chain event**: diff-review
   `state.rs`/`eventloop.rs` on each bump (single pseudonymous maintainer +
   AI-agent bulk commits).
3. **Keep the fork 100% behind our facade.** We currently re-export
   `MqttOptions`/`QoS`/`Transport` (core/src/lib.rs) — these must become our
   own types (already a 0.3 item) or the fork's breaking-change cadence
   (22 breaking entries across 12 releases in ~3.5 months) leaks to our users.
4. **Contingency**: Apache-2.0 (verified) — if the maintainer vanishes, we
   maintain a pinned vendored copy; same end-state as "own fork" from a much
   better base.

## Top risks

1. **Bus factor = 1, pseudonymous** ("eagle" <lefttolive@proton.me>, 445/445
   fork commits, AI-agent-driven workflow). Hard fork — crate split + broker
   removal makes upstreaming to bytebeamio impossible; upstream itself is
   near-dormant (1 commit in 8 months, 168 open issues).
2. **crates.io lags HEAD** (see mitigation 1).
3. **Breaking-change treadmill** (see mitigation 3).
4. **Verification gaps**: zero fuzzing/proptest on codecs; real-broker CI is
   Linux-only mosquitto, silently skipped if absent; ~50% of spec requirements
   marked unreviewed. Source is slop-clean (0 dead_code/TODO/ignored tests);
   the mess is repo-rim only (agent-session scratch files).
5. **The maintainer ports our crate** (see below) — coordination opportunity
   and fork-of-us risk at once.

## Adoption check (2026-07-08): small but real, incl. one heavyweight

GitHub code search (Cargo.toml declarations): ~20 repos. Notable:
**rustfs/rustfs (29.6k stars)** uses `rumqttc-next` for its MQTT bucket-event
notification target — and imports `Broker` and `PublishNoticeError`, i.e. it
adopted the fork *for the tracked-notice API*, same motivation as ours.
Also procivis/one-core (commercial, digital identity), cool-japan/scirs
(267 stars), plus small IoT projects. Daily download pattern shows a clear
weekday/weekend CI rhythm — honest machine-heavy numbers, not bot inflation
and not mass adoption. `rumqttc-v5-next` is rarely used directly; consumers
take the `rumqttc-next` facade (= **v5**) or `rumqttc-v4-next` (= MQTT 3.1.1;
the one we need today).

## mqtt-typed-client-next — the maintainer's port of this library

Created 2026-07-06: a squashed re-import of our exact v0.2.0 source, ported to
`rumqttc-v4-next`/`-v5-next` (path deps on his repo checkout; NOT published —
blocked on his own unreleased `PublishOptions` API). Intent per his TODO.md:
tracking port, kept only "unless upstream mqtt-typed-client has adopted the
same API" — explicitly provisional, not (yet) a competitor.

- **Attribution**: sloppy but not hostile. Preserves our `authors`,
  `repository`, `license` metadata and README links, but **deletes the
  LICENSE-MIT/LICENSE-APACHE files from all 5 locations** — the one real
  compliance gap (raised with him in the coordination issue).
- **Validated migration recipe** (macros + mqtt-topic-engine: zero changes;
  no trait abstraction needed — feature-gated `use rumqttc_v4 as rumqttc;`
  works because the -next crates share an API shape):
  `AsyncClient::builder(opts).capacity(cap).build()`;
  `MqttOptions::new(id, Broker::tcp(host, port))`; `set_keep_alive` takes
  seconds; `publish(topic, payload, PublishOptions::new(qos).retained())`;
  incoming `Publish.topic` is `Bytes` (he uses `from_utf8_lossy` — we should
  reject/log non-UTF-8 instead, and consider byte-level router matching to
  avoid the per-message alloc); v4/v5 packet-shape match arms; per-backend
  rustls re-exports; `T: Sync` bound on publishers. Plus: MSRV 1.89,
  edition 2024.
- **Tracked-notice API: NOT wired into the typed layer at all** — our Tier-1
  ack-surfacing design (publish→ack future, SubAck, MessageMeta) remains
  unbuilt, undisputed territory.
- **Worth upstreaming to us regardless of backend choice**: his
  QoS-downgrade-on-unsubscribe feature (TopicRouter returns
  `UnsubscribeAction::{NoBrokerAction, Unsubscribe, Resubscribe{qos}}`) —
  implemented via our own parked `get_max_qos_for_topic` helper (our TODO,
  completed), with 6 focused router tests.
- Losses in his port: our workspace integration tests deleted without
  replacement; `.github/` CI, deny.toml, rustfmt.toml removed; Bincode
  swapped for Wincode (breaking).

## Migration effort for us: S–M (~1 day of code) + a 0.3-level semver bump

`rumqttc = { package = "rumqttc-v4-next", ... }` — lib target is still named
`rumqttc`, imports unchanged. Behavioral break to design around:
`mqtts://`/`ssl://`/`wss://` URL schemes are rejected (needs an
explicit-transport story for our URL-based TLS users). `use-rustls` defaults
to aws-lc (heavier build). `mqtt-topic-engine`'s rumqttc-interop feature needs
a `-next` variant to keep upstream-rumqttc users working.
