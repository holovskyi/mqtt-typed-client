# §4 Connection State Observability — implementation plan

*Sub-plan of [PLAN_0.3.md](./PLAN_0.3.md) step 5. Drafted 2026-07-09. Forks
resolved by an unbiased Fable-5 decision pass (2026-07-09), grounded in the
current tree.*

> **DONE 2026-07-10.** Shipped in two commits: `fix(core): close subscriber
> channels on terminal event-loop death` (piece 1) then `feat(core): observable
> connection state via watch channel` (piece 2). One deviation from the plan
> below, prompted by the code-critic: `Reconnecting { attempt }` uses a SEPARATE
> `reconnect_attempt` counter, not `error_count`. Resetting `error_count` on a
> reconnect CONNACK (as an earlier draft did) made a flapping broker immortal
> (never hits `MAX_CONSECUTIVE_ERRORS`) and pinned backoff at the 100ms floor.
> The two-counter split keeps `error_count` driving backoff+termination
> (unchanged from pre-feature) while `reconnect_attempt` (resets on ANY
> successful poll) feeds the observable state. All else landed as planned.

## Goal

Make the connection lifecycle **observable** (a `watch` channel of typed state)
AND make event-loop death **correct** (no more zombie consumers parked on
`receive().await` forever). Every public type is v5-first, `#[non_exhaustive]`,
protocol-neutral, additive-only — consistent with the rest of 0.3.

Two independently-shippable pieces, in this order:

1. **Zombie-consumer bugfix** (self-contained, lands FIRST, own commit).
2. **`ConnectionState` observability feature** (own commit, builds on 1).

---

## Resolved forks (Fable-5 decision pass, 2026-07-09)

- **Fork 1 — channel type: `tokio::sync::watch`.** State is level-triggered
  ("what is it now / when does it change"), not a transition log. Collapsing
  `Reconnecting{1}→{2}` loses nothing actionable; terminal `Disconnected` can
  never be missed (watch retains the last value). `broadcast` would re-introduce
  the `RecvError::Lagged`-as-`Err` footgun that §2b/§5 deliberately removed from
  the message path — incoherent to put it back on the state path. Users needing
  an unmissed transition log use `tracing`.
- **Fork 2 — payload: dedicated `#[non_exhaustive]` enums**, no `String`, no
  embedded backend error. `attempt: u32`; `Connected { session_present: bool }`.
  (Full shape below.)
- **Fork 3 — exposure: `MqttClient` only.** `MqttClient` is `Clone` and is what
  reacting tasks hold; `MqttConnection` is not `Clone` and is consumed by
  `shutdown(self)`. Accessor returns an owned clone (needed — `changed()` takes
  `&mut self`). Adding it to `MqttConnection` later is additive if a need appears.
- **Fork 4 — zombie fix sequencing: SEPARATE commit, BEFORE the feature.** It is
  a correctness fix that already bites on 0.2-as-shipped; keeping it out of the
  feature diff means it is not hostage to `ConnectionState` design iteration.

**Accepted trade-off (Fork 3):** this puts `tokio::sync::watch::Receiver` in the
public API. The de-leak campaign targeted **rumqttc**, not tokio (the crate is
already publicly tokio-shaped); wrapping watch's `borrow`/`changed`/`wait_for`
surface costs far more than it buys.

**Plan-imprecision correction (from the decision pass):** PLAN_0.3.md §4 pins the
zombie bug to the `MAX_CONSECUTIVE_ERRORS` break (async_client.rs:207-214). That
is incomplete — the server-initiated `Incoming(Disconnect)` break (`:188-191`)
is equally terminal and equally uncleaned. The fix must be a **single cleanup
trigger after the loop**, covering all three exit paths, not a per-break patch
(altitude: generalize the mechanism, don't special-case each break — per
CLAUDE.md).

---

## Piece 1 — Zombie-consumer bugfix (commit 1, lands first)

### The bug (verified in code)

`SubscriptionManagerActor::run()` (subscription_manager.rs:215-256) exits its
`select!` loop only on (a) the `shutdown_rx` oneshot (fired by
`SubscriptionManagerController::shutdown()`, which lives in `MqttConnection`), or
(b) the command channel closing. On exit it runs
`cleanup_active_subscriptions()` (`:363`) → `topic_router.cleanup()` closes each
subscriber channel → `receive()` yields `None`.

The event-loop task (`MqttClient::run`, async_client.rs:133-230) holds only a
`SubscriptionManagerHandler` clone — NOT the controller. When that loop
terminates (any of the three `break`s), the actor is never told: the command
channel stays open because long-lived `MqttClient` clones each hold a handler.
So the actor keeps running, cleanup never fires, and every consumer parks on
`receive().await` forever. **Only `MqttConnection::shutdown()` currently cleans
up — terminal event-loop death does not.**

### The fix (single trigger, all exit paths)

Add a terminal `Command::Shutdown` to the actor command enum
(subscription_manager.rs:115). In the actor `run()` match, that arm `break`s the
loop (falling through to the existing `cleanup_active_subscriptions()` at `:255`
— reuse, no new cleanup code). Add a `pub(crate) async fn shutdown(&self)` to
`SubscriptionManagerHandler` (`:665`) that sends it (best-effort; a closed
channel means the actor already exited — harmless).

The trigger lives **inside `run()`, right after the `loop`** (NOT in the
spawning task). This is forced AND cleaner: `run(mut event_loop, handler)` takes
the handler **by value** (async_client.rs:135) and the spawned task moves its
`handler_clone` INTO the call (`:88`), so `handler_clone` is unusable after the
`.await` — the plan's earlier "after `Self::run` returns" wording would not
compile. Doing it inside `run()` also co-locates it with the terminal
`Disconnected` publish (Phase b, which also needs `state_tx` owned by `run()`),
making the "publish `Disconnected` → then `shutdown()`" ordering trivially
correct (one scope). Concretely: after the `loop { … }` in `run()`, once (piece
2 lands) the `Disconnected` state is published, call
`subscription_manager.shutdown().await`. This covers **all three** loop-exit
paths in one place:
- `Outgoing(Disconnect)` (`:193`) — clean shutdown; the controller already
  cleaned up first, so this is a harmless idempotent no-op (actor already gone).
- `Incoming(Disconnect)` (`:188`) — server DISCONNECT; previously leaked.
- `MAX_CONSECUTIVE_ERRORS` (`:207`) — previously leaked.

(For piece 1 shipping standalone before piece 2, `run()` still owns the handler,
so the `shutdown().await` goes at the end of `run()` unconditionally — no
`state_tx` yet.)

**Idempotence note:** on `MqttConnection::shutdown()` the controller's
`shutdown_tx` fires first and the actor exits; the later `Command::Shutdown` send
from the event-loop task then no-ops on a closed channel. No double-cleanup, no
panic.

### Tests (piece 1)

- Integration/unit: after simulated terminal event-loop death, a subscriber's
  `receive().await` returns `None` (not a hang). **Needs a NEW test harness**:
  the existing scaffold (`make_actor`, `:725-730`) DROPS `command_tx` and
  `shutdown_tx` and calls `handle_send`/`handle_slow_send` directly — it never
  spawns `run()`'s select loop, so it cannot exercise
  `Command::Shutdown → break → cleanup`. Add a variant that keeps `command_tx`,
  spawns `actor.run()`, sends `Command::Shutdown`, and awaits the join handle,
  then asserts the subscriber channel is closed. (`cleanup_active_subscriptions`
  calls `self.client.unsubscribe()` on the dummy `AsyncClient` whose event loop
  was discarded — that just enqueues on the request channel (cap 10) and returns
  `Ok` without a running loop, so it won't hang for a small topic count.)
- Regression: normal `MqttConnection::shutdown()` still returns `Ok(())` and
  does not double-panic.

---

## Piece 2 — `ConnectionState` observability (commit 2)

### Phase a — the types (new module `core/src/connection_state.rs`, re-export at crate root)

```rust
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is up. `session_present` = broker resumed an existing
    /// session (v4 CONNACK today; v5 CONNACK identically later).
    #[non_exhaustive]
    Connected { session_present: bool },

    /// Connection lost; the backend is retrying. `attempt` is the number of
    /// **consecutive poll failures** (the same counter that trips
    /// `MaxErrorsExceeded`), not a distinct "reconnect attempt" number; it
    /// resets to 0 on the next successful poll.
    #[non_exhaustive]
    Reconnecting { attempt: u32 },

    /// Terminal. The event loop has exited; no further transitions will occur
    /// and all subscribers' `receive()` now yield `None`.
    #[non_exhaustive]
    Disconnected { reason: DisconnectReason },
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    /// `MqttConnection::shutdown()` was called (Outgoing Disconnect path).
    CleanShutdown,

    /// The broker sent a DISCONNECT (Incoming Disconnect path).
    /// The v5 disconnect reason code arrives here additively in 0.4.
    #[non_exhaustive]
    BrokerDisconnected {},

    /// The event loop terminated after too many consecutive poll errors.
    #[non_exhaustive]
    MaxErrorsExceeded { errors: u32 },
}
```

- **Own enums, not `String`/backend error.** Matches the house style in
  `core/src/client/error.rs` (`ConnectReasonCode`, `ConnectionEstablishmentError`
  — own `#[non_exhaustive]` protocol-neutral enums). A `String` is unmatchable;
  embedding a backend error would force `Arc`/re-leak backend detail onto a
  channel whose job is *state*, not diagnostics (the last error already goes to
  `tracing` at the break site).
- **`#[non_exhaustive]` on the struct-like variants too** (incl. the
  awkward-but-legal `BrokerDisconnected {}`) so v5 fields (reason code,
  properties) are addable without breaking — variant-granularity additive growth.
- **`BrokerRejected` deliberately NOT shipped yet.** Post-reconnect CONNACK
  rejection currently surfaces inside rumqttc's `ConnectionError` (`Err` arm) and
  counts toward `MAX_CONSECUTIVE_ERRORS`; distinguishing it is additive later
  (the enum is `#[non_exhaustive]` precisely for this).
- Derive `PartialEq, Eq` — cheap, and lets consumers `wait_for(|s| *s == ...)`.

### Phase b — plumbing (async_client.rs)

1. **Capture `session_present` from the bootstrap CONNACK.**
   `establish_connection` (`:100-129`) currently discards it via
   `ConnAck { code, .. }`. Change its success return to carry it (e.g. return a
   small `struct Established { event_loop: EventLoop, session_present: bool }`,
   or a tuple `(EventLoop, bool)` — a named struct per CLAUDE.md tuple policy).

2. **Create the watch channel in `connect_with_config`**, seeded with the real
   initial state:
   ```rust
   let (state_tx, state_rx) =
       tokio::sync::watch::channel(ConnectionState::Connected { session_present });
   ```
   Move `state_tx` into the spawned event-loop task (→ into `run()`, which owns
   it for the whole lifetime and publishes the terminal `Disconnected`); store
   `state_rx` in the `MqttClient` struct (new field, `Clone`-friendly).
   **Invariant (needs a `// why` comment on the field):** the stored `state_rx`
   is a *seed* handle that is NEVER polled (`changed()`/`borrow_and_update()`).
   It stays frozen at the initial watch version, so every `connection_state()`
   clone inherits that frozen version — a task subscribing AFTER a terminal
   transition still sees `Disconnected` on its first `changed()`/`borrow()`.
   Polling the stored handle would advance it and make late subscribers miss the
   terminal state.

3. **`Self::run` gains a `state_tx: watch::Sender<ConnectionState>` param** and
   publishes at each transition (`send_replace`/`send` — ignore "no receivers"
   errors; observability is best-effort):
   - reconnect-without-session arm (`:146`) → `Connected { session_present: false }`
     (after the resubscribe attempt).
   - reconnect-with-session arm (`:161`) → `Connected { session_present: true }`.
   - `Err` arm (`:203`) → `Reconnecting { attempt: error_count }` (publish after
     incrementing `error_count`, before the terminal check).
   - **Terminal (single publish point, mirrors the single cleanup trigger):** at
     each `break` set a local `let reason = DisconnectReason::…;` instead of a
     bare `break`, then AFTER the loop publish
     `state_tx.send(ConnectionState::Disconnected { reason })` once. Reasons:
     `Outgoing(Disconnect)` → `CleanShutdown`; `Incoming(Disconnect)` →
     `BrokerDisconnected {}`; `MAX_CONSECUTIVE_ERRORS` →
     `MaxErrorsExceeded { errors: error_count }`.
   - Ordering at end of task: publish `Disconnected` → then the piece-1
     `handler_clone.shutdown().await`. (State visible before consumers observe
     `None`; either order is defensible, this one lets a state watcher react
     before the message streams close.)

### Phase c — exposure (async_client.rs, `MqttClient`)

```rust
impl<F> MqttClient<F> {
    /// Watch the connection lifecycle. Returns an independent receiver
    /// pre-seeded with the current state.
    ///
    /// `Disconnected` is **terminal** — once observed, `changed()` never fires
    /// again. Prefer a `loop { rx.changed().await?; if matches!(*rx.borrow(),
    /// Disconnected { .. }) { break } }` shape over spinning on `changed()`.
    pub fn connection_state(
        &self,
    ) -> tokio::sync::watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }
}
```

- New field `state_rx: watch::Receiver<ConnectionState>` on `MqttClient`
  (preserves `Clone`). Exactly **two** construction sites carry it (grep-verified,
  no others): `connect_with_config` `Self { … }` (`:90`) and
  `clone_with_custom_serializer` `MqttClient { … }` (`:381`, carries
  `state_rx: self.state_rx.clone()` — the serializer-swapped client shares the
  same connection, so it shares the same state channel).
- **Re-export sites:** add `ConnectionState`/`DisconnectReason` to the crate-root
  re-export block in `core/src/lib.rs` (mirror the `MqttConnection` export) AND
  to the umbrella `prelude` in `src/lib.rs` for discoverability.

### Phase d — example, CHANGELOG, docs

1. **Example** `examples/010_connection_state.rs`: spawn a task that loops
   `state_rx.changed().await` and logs each transition; main publishes/subscribes
   normally, then `shutdown()`; show the watcher observing `Connected →
   Disconnected { CleanShutdown }`.
2. **CHANGELOG — Added:** `MqttClient::connection_state()` +
   `ConnectionState`/`DisconnectReason`. **Fixed:** terminal event-loop death now
   closes subscriber channels so `receive()` yields `None` instead of hanging
   forever (piece 1 — this is a real bugfix, call it out).
3. **Docs:** note it is `watch` (latest-state, transitions may collapse; terminal
   is never missed), and that `Disconnected` is terminal. Wire example 010 into
   `src/lib.rs` + `examples/README.md` (mirror the 009 pattern).

### Tests (piece 2)

- Unit: initial state after `connect` is `Connected { session_present }` (seeded
  from bootstrap CONNACK).
- Behavior: after `shutdown()`, a `connection_state()` receiver observes
  `Disconnected { CleanShutdown }`.
- (If feasible in the harness) a simulated terminal error yields
  `Disconnected { MaxErrorsExceeded { .. } }`.

---

## Open items / assumptions (flag for critic)

1. **`Reconnecting` timing is approximate.** rumqttc reconnects internally; we
   infer "reconnecting" from the `Err` arm and "connected" from the next
   successful `ConnAck`. Intermediate `Ok(notification)` events between them do
   not reset the displayed `Reconnecting` (only a real success does). Acceptable
   for observability; documented.
2. **`send` never actually errors in practice.** There is ALWAYS a live receiver
   — `state_rx` is stored in `MqttClient` and handed to the user immediately, so
   `send` only errors once *every* `MqttClient` clone is dropped. Early publishes
   (`Reconnecting`, etc.) are therefore **retained**, not silently dropped. Still
   ignore the `send` error defensively (client fully dropped mid-shutdown), but
   the rationale is "client gone", not "no receivers yet".
3. **Seed value assumes bootstrap success.** `connect()` only returns `Ok` after
   a successful CONNACK, so seeding `Connected` is always correct at construction.
4. **Negative decision still holds:** `ConnectionState` carries NO resubscribe /
   subscription-failure info (that is §2c, on the per-subscriber `receive()`
   stream). `Connected { session_present }` is connection-scoped, not
   subscription-scoped — consistent with the locked decision.
5. **Stretch (`ReconnectPolicy`) stays out** of this plan; the watch channel is
   its prerequisite and now exists.
