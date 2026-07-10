# 0.4 Plan (draft)

*Split out of [PLAN_0.3.md](./PLAN_0.3.md) on 2026-07-10. These sections were
designed during the 0.3 cycle but **deferred to 0.4**: clean addressable SubAck
correlation requires the `rumqttc-v4-next` backend, whose audited fixes are not
yet on crates.io. 0.3 shipped upstream-only.*

**The gating chain:** §6 backend swap → unblocks §3 SubAck surfacing (typed,
addressable) → unblocks §2c resubscribe-failure surfacing. All three land
together in 0.4 on the next backend; log-only half-measures on upstream were
rejected (YAGNI — they would be rewritten). See the feasibility record in the
0.3 research memory and
[research/RUMQTTC_NEXT_AUDIT_2026.md](./research/RUMQTTC_NEXT_AUDIT_2026.md).

**Backend-neutral design constraint (locked):** the public ack surface must NOT
leak `rumqttc-v4-next` types (`SubscribeNotice` etc.). Expose our own
`#[non_exhaustive]` types (`ReceiveEvent::SubscriptionLost`, a v5-shaped publish
result); the backend stays behind the facade so a future backend switch (or the
mqtt5 opt-in) is not a breaking change. This is why §1 de-leak shipped first in
0.3.

**Dependencies from 0.3 (already shipped):** §2b `ReceiveEvent` enum
(`#[non_exhaustive]`, delivered per-subscriber) is the home for the new
`SubscriptionLost` variant; §4 connection-state watch channel is live; the §1
facade is in place.

---

## 3. Ack surfacing

- **SubAck (minimal, backend-independent):** stop dropping
  `SubAck.return_codes` — surface broker-side subscription rejection and QoS
  downgrade (log + typed event/error to the subscriber).
- **Must cover the RECONNECT path, not only `subscribe()`** (from r/rust
  feedback, verified 2026-07-09). The initial-`subscribe()` surfacing and the
  resubscribe-after-reconnect surfacing read the same `SubAck.return_codes`;
  scoping SubAck to `subscribe()` only leaves the reconnect hole open. Honest
  resubscribe-failure detection (§2c) *requires* this — so §3-on-the-reconnect-path
  GATES §2c.
- **Full correlation (publish → PUBACK/PUBCOMP future, subscribe → granted
  QoS)** rides on the rumqttc-next backend (`publish_tracked`/`subscribe_tracked`,
  returning a `SubscribeNotice` that correlates the SUBACK itself and resolves
  per-filter failures as a typed error). Public shape: `publish()` returns a
  future resolving to a v5-shaped result (reason code; `Success` on v4). Design
  the public API backend-neutral; wire it to the notice layer when the backend
  lands.
- `PublishOptions` builder (`#[non_exhaustive]`, private fields):
  qos + retain now; v5 knobs (expiry, user properties, content type) arrive
  additively later. Replaces loose (qos, retain) args where public.

> **Feasibility note (2026-07-10, read-only audit of rumqttc 0.25.1):** on
> upstream, `client.subscribe().await` returns `Ok(())` with no pkid, and SUBACK
> carries `pkid + return_codes` but not the topic filters — so addressable typed
> delivery is only achievable via a fragile FIFO `pkid → topic` correlation over
> `Outgoing::Subscribe(pkid)` that rests on unenforced invariants and cannot
> signal a subscribe lost on reconnect. Rejected as the hack CLAUDE.md forbids.
> The clean path is `rumqttc-v4-next`'s `subscribe_tracked() -> SubscribeNotice`
> (§6).

## 2c. Resubscribe-failure surfacing (from r/rust feedback, verified 2026-07-09)

**The real reconnect gap.** After a session-less reconnect we call
`resubscribe_all()`, but its result is invisible and never acted on. Three
stacked defects (`subscription_manager.rs:479-511`, `async_client.rs:153-158` —
line numbers from the 0.3 tree; re-verify):

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

**Home: the `ReceiveEvent` enum (§2b, shipped in 0.3), NOT `ConnectionState`.**
It is already `#[non_exhaustive]`, already carries this class of "stream alive
but you lost something" event (`Lagged`), is delivered to exactly the affected
subscriber, and has no external users yet. Add a variant (name a strawman):

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
(b) bounded retry with backoff then notify. Lean (a) for the 0.4 landing —
without SubAck confirmation a retry cannot tell success from failure, so it is
just a louder no-op. At minimum: do not silently continue.

## 6. Backend switch to rumqttc-v4-next (decision: adopt-with-mitigations)

- Swap `rumqttc` → `rumqttc-v4-next` **pinned to an audited git rev / next
  audited release** (crates.io 0.33.2 lacks audited fixes — see audit doc).
  Migration recipe validated by the maintainer's port (S–M, ~1 day):
  builder construction, `Broker::tcp`, `PublishOptions`, `Bytes` topics
  (reject non-UTF-8, do NOT `from_utf8_lossy`), explicit-transport story for
  `mqtts://`/`wss://` URLs. MSRV → 1.89, edition 2024 implications.
- Timing gate: coordinate with "eagle" (mqtt-typed-client-next#1) first; his
  response may add a QoS-downgrade PR and a crates.io release. **The hard blocker
  is a crates.io release** carrying the `[Unreleased]` work — `cargo` forbids
  publishing a crate with a git dependency (even optional/feature-gated), so
  until the next backend is on crates.io we cannot publish 0.4 against it.
- **Expose behind a feature flag**, default backend = upstream rumqttc (stable,
  on crates.io); next behind an opt-in feature with backend-neutral public ack
  types (feature switches the impl, not the public API — avoids semver churn).
  `mqtt-topic-engine`'s rumqttc-interop feature gains a `-next` variant so
  upstream-rumqttc users keep working.
- **Fallbacks if eagle never publishes** (bus factor 1, pseudonymous):
  (a) own-publish the next fork under our crate name (audit contingency #4 —
  cheapest, keeps our API, Apache-2.0 verified) ≫ (b) migrate to `mqtt5`
  (LabOverWire/mqtt-lib — full rewrite, swaps one bus-factor-1 dep for another,
  but IS on crates.io). mqtt5 = 0.4+ experimental opt-in only, not primary.
  Full multi-backend trait abstraction = premature (both audit docs), don't
  build yet.
- This is also what answers the user-facing "what happens to a publish issued
  mid-outage?" question: inflight QoS 1/2 replay and offline queueing live in
  rumqttc's `EventLoop`/`state.rs`, and the `-next` audit
  (`research/RUMQTTC_NEXT_AUDIT_2026.md:25-28`) confirms reconnect retransmission
  with notice senders preserved + `SessionReset` on session loss. On upstream
  rumqttc today we cannot honestly state the outcome; §6 makes it answerable. (An
  own managed offline queue stays out of scope — see the standing negative
  decision in `research/CLIENT_LIBRARY_LANDSCAPE_2026.md`.)

## Order of work (0.4)

1. §6 backend swap to `rumqttc-v4-next` behind a feature flag (gated on the
   eagle coordination outcome + the next backend reaching crates.io).
2. §3 typed SubAck surfacing on the next backend (`subscribe()` AND the
   reconnect/resubscribe path) via `subscribe_tracked`/`SubscribeNotice`,
   surfaced through backend-neutral public types.
3. §2c resubscribe-failure surfacing — `ReceiveEvent::SubscriptionLost` on the
   affected subscriber; gated on step 2. Touches `mqtt-topic-engine`
   (return-type change → version bump).
4. `PublishOptions` builder + publish-ack future (v5-shaped result).

## Open items (external)

- eagle's response to mqtt-typed-client-next#1 (coordination). Pinged 2026-07-10
  with the crates.io-release timeline question; answered our upstream #1056/#1057
  but not yet the coordination issue.
- Fallback ladder above if the coordination stalls.
- LabOverWire/mqtt-lib#100 RESOLVED (broker gated behind `broker` feature, ships
  mqtt5 0.36.0) — positive signal for mqtt5 as a 0.4+ opt-in backend.
