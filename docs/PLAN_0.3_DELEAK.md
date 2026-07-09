# 0.3 §1 — De-leak the Public API: Detailed Plan

> **STATUS: IMPLEMENTED 2026-07-09** — commits `c6e1ca2` (W1), `1251c11`
> (W2), `5dc3480` (W3+W4, merged: example 004 requires the TLS plumbing to
> land with the facade), `9075516` (W5), `05e61d8` (post-review fixes).
> Post-implementation critic verdict: ship (after fixes). Remaining plan text
> below is kept as the design record.

*2026-07-09. Expands PLAN_0.3.md §1. Input: exhaustive leak map (agent audit)
+ DUAL_PROTOCOL_API_DESIGN_2026.md locked decisions. Revised same day after a
critic pass (verdict: fix-then-ship) — all BLOCKER/MAJOR findings folded in.
Guiding decision (Artem, 2026-07-09): **minimal facade, not a universal
builder** — cover the ~90% surface our examples exercise, everything else goes
through an explicitly semver-exempt backend escape hatch.*

## Leak inventory (verified 2026-07-09)

| # | Leak | Where | Depth |
|---|------|-------|-------|
| L1 | `MqttClientConfig.connection: rumqttc::MqttOptions` | config.rs:188 | public field |
| L2 | `from_url() -> Result<_, rumqttc::OptionError>` | config.rs:208 | return type |
| L3 | `rumqttc::QoS` | last_will.rs:10,29; publisher.rs:40,57; subscription_builder.rs:98; subscription_manager.rs:36 | fields + signatures |
| L4 | `ConnectionEstablishmentError::Network(Box<rumqttc::ConnectionError>)` | error.rs:27 | enum variant |
| L5 | `BrokerRejected { code: rumqttc::ConnectReturnCode }` | error.rs:34-37 | enum field |
| L6 | `MqttClientError::ClientOperation(#[from] rumqttc::ClientError)` | error.rs:61 | variant + From |
| L7 | `MqttClientError::Configuration(#[from] rumqttc::OptionError)` | error.rs:65 | variant + From |
| L8 | `pub use rumqttc::{MqttOptions, QoS, Transport}` | core/src/lib.rs:153-159 (+ preludes) | re-export |
| L9 | `pub use rumqttc::tokio_rustls` / `::rustls` | src/lib.rs:31,36 | re-export (narrows, see W4) |
| L10 | root features named `rumqttc-*` | Cargo.toml:73-80 | feature names |
| L11 | examples/doc-tests import `rumqttc::QoS`; crate doc-test drives `MqttOptions` setters (core/src/lib.rs:36-39); `ClientSettings.event_loop_capacity` doc names rumqttc (config.rs:74) | examples 002, internal/*, config.rs:230 | docs |
| L12 | `MqttPublisher::new(client: rumqttc::AsyncClient, ..)` is `pub` on a re-exported type | publisher.rs:25 | signature |
| L13 | `pub mod subscription_manager` exposes `SubscriptionManagerActor::spawn(AsyncClient, ..)` + `Command<T>` | routing.rs:10 | module visibility |

Already clean: `macros/` (zero direct rumqttc references), engine `QoS`
(mqtt-topic-engine/src/qos.rs — derives are a strict superset of
rumqttc::QoS's, both `repr(u8)` with identical discriminants, conversions both
ways already exist; **no engine change or release needed**), `ClientSettings`,
serializers, topic types, connect signatures.

## Work packages (each = one commit, compiles + tests green, in this order)

### W1. QoS swap (mechanical, self-contained)

- Re-export engine `QoS` as the crate's `QoS` (replaces L8's rumqttc::QoS).
- Replace `rumqttc::QoS` in every public position (L3). Conversion to rumqttc
  happens only at the backend boundary (publisher submit path,
  subscription_manager.rs:288 — already there).
- Fix examples + doc-tests (L11 QoS part) to `use mqtt_typed_client::QoS`.
- Visibility fixes while touching these files: `MqttPublisher::new` →
  `pub(crate)` (L12); `subscription_manager` module → private, keep only the
  `SubscriptionConfig` re-export via routing.rs (L13).

### W2. Own the errors

New types in `core/src/client/error.rs` (all `#[non_exhaustive]`):

```rust
/// v5-superset reason codes; v4 ConnectReturnCode maps into a subset.
#[non_exhaustive]
pub enum ConnectReasonCode {
    Success,
    UnspecifiedError,            // v5-only
    ProtocolError,               // v5
    UnsupportedProtocolVersion,  // v4: RefusedProtocolVersion
    ClientIdentifierNotValid,    // v4: BadClientId
    BadUserNamePassword,         // v4: BadUserNamePassword
    NotAuthorized,               // v4: NotAuthorized
    ServerUnavailable,           // v4: ServiceUnavailable
    ServerBusy, Banned, UseAnotherServer, ServerMoved, // v5-only
    // + remaining v5 CONNACK codes, added now, #[non_exhaustive] guards the rest
}

/// Opaque backend error: preserves Display + source chain without naming rumqttc.
pub struct BackendError(Box<dyn std::error::Error + Send + Sync>);
```

- L5: `BrokerRejected { code: ConnectReasonCode }` + internal
  `From<rumqttc::ConnectReturnCode>`.
- L4: `Network(#[source] BackendError)`. In the (now private) conversion from
  `rumqttc::ConnectionError`, special-case
  `ConnectionError::ConnectionRefused(code)` → `BrokerRejected` so a broker
  rejection has ONE representation regardless of which code path saw it.
- L6: replace `#[from] rumqttc::ClientError` with our own
  `ClientOperationError` (`RequestChannelClosed`, `RequestChannelFull`) — no
  public `From`. Honesty note: rumqttc's `ClientError::TryRequest` discards
  the flume Full/Disconnected discriminant, so `TryRequest →
  RequestChannelFull` is an approximation; our code never calls `try_*`
  today, so the variant is unreachable — document the approximation at the
  mapping site. (No `Backend(..)` variant in 0.3 — `#[non_exhaustive]` lets
  0.4 add it if the -next backend needs it.)
- L7: disappears together with L2 — URL parsing becomes ours (W3); errors are
  our own `UrlParseError` (`#[non_exhaustive]`: scheme, host, port, params,
  protocol variants).
- All existing public error enums get `#[non_exhaustive]` (locked-in-0.3 item 4).

### W3. Connection facade (the big one)

New `core/src/client/connection_options.rs`:

```rust
pub struct ConnectionOptions {           // manual Debug (prints tweak count), Clone
    pub client_id: String,
    pub host: String,
    pub port: u16,
    pub keep_alive: Duration,
    pub credentials: Option<Credentials>,        // own struct { username, password: String }
    pub session: SessionPolicy,                  // v5-shaped, see below
    pub protocol: ProtocolVersion,               // #[non_exhaustive], default V4
    pub transport: Transport,                    // own enum, see W4
    backend_tweaks: Vec<Arc<dyn Fn(&mut BackendOptions<'_>) + Send + Sync>>, // private, ALWAYS present
    // NOT mirrored: inflight, request channel sizes, pending throttle,
    // packet-size caps, websocket headers, proxy, ... -> escape hatch only.
}
// Constructors (pub fields + one private field => no struct literal for users):
//   ConnectionOptions::new(client_id, host, port)  — defaults for the rest
//   ConnectionOptions::from_url(url)               — see URL grammar below
// Style in examples: let mut opts = ConnectionOptions::new(..); opts.keep_alive = ..;

#[non_exhaustive]
pub enum SessionPolicy {
    CleanPerConnection,          // v4: clean_session = true  | v5: clean_start=true  + expiry 0
    Resume,                      // v4: clean_session = false | v5: clean_start=false + expiry u32::MAX ("never expires", MQTT 5 §3.1.2.11.2)
    ResumeFor(Duration),         // v4: CONNECT-TIME ERROR    | v5: clean_start=false + expiry = d
                                 // d rounds UP to whole seconds; d > (u32::MAX-1) secs -> connect-time error (no silent saturation)
}

#[non_exhaustive]
pub enum ProtocolVersion { V4, V5 }   // V5 in 0.3 -> clean "arrives in 0.4" connect error
```

- `MqttClientConfig.connection: ConnectionOptions` (kills L1).
- Conversion `ConnectionOptions -> rumqttc::MqttOptions` is one private
  function at connect time (async_client.rs) — the single choke point the 0.4
  `BackendClient` enum will branch on. **The conversion VALIDATES instead of
  panicking** (rumqttc setters assert): `keep_alive` in (0, 1s) → error;
  `SessionPolicy::Resume`/`ResumeFor` with empty `client_id` → error
  (rumqttc `set_clean_session` asserts non-empty id); both surfaced as
  `MqttClientError::ConfigurationValue`.
- `with_last_will` keeps its `TypedLastWill` API; the rumqttc `LastWill` is
  built inside the private conversion (LWT payload serialization stays eager).
- **URL parsing becomes ours** (kills L2/L7), on the `url` crate. Grammar:
  - schemes `tcp|mqtt|ssl|mqtts|ws|wss` — ALL accepted at parse time
    (transport availability is checked at connect, see W4);
  - default ports: keep rumqttc-compat (`mqtt/tcp`→1883, `mqtts/ssl`→8883,
    `ws`→8000, `wss`→8000 — yes, 8000: documented compat quirk);
  - query params kept: `client_id`, `keep_alive_secs`, `clean_session`
    (maps to `SessionPolicy`), userinfo credentials; NEW: `?protocol=4|5`.
  - query params rumqttc accepted but the facade does NOT mirror
    (`inflight_num`, `request_channel_capacity_num`, `max_request_batch_num`,
    `pending_throttle_usecs`, `max_incoming_packet_size_bytes`,
    `max_outgoing_packet_size_bytes`): **explicit `UrlParseError` with a
    "moved to the backend escape hatch" message** — NOT silently ignored, NOT
    carried. This is deliberately not a superset; migration table documents
    each param. Unknown params error, as in rumqttc.
- **Escape hatch** (semver-exempt), method behind non-default feature
  `unstable-backend-api`, field always present:

```rust
/// Argument wrapper so 0.4 can add V5 additively even inside the exempt zone.
#[non_exhaustive]
pub enum BackendOptions<'a> {
    V4(&'a mut rumqttc::MqttOptions),
    // V5(&'a mut rumqttc::v5::MqttOptions) arrives with the 0.4 BackendClient
}

#[cfg(feature = "unstable-backend-api")]
impl ConnectionOptions {
    /// Applied to backend options AFTER facade conversion, at connect time,
    /// in insertion order. SEMVER-EXEMPT: may change with any backend change.
    pub fn backend_tweak(&mut self, f: impl Fn(&mut BackendOptions<'_>) + Send + Sync + 'static) -> &mut Self
}
```

  The feature also re-exports the backend crate as
  `mqtt_typed_client::backend::rumqttc` so hatch users don't need their own
  version-matched rumqttc dependency.
- Crate-level doc-test (core/src/lib.rs:36-39) migrates to the facade IN THIS
  COMMIT (it gates "compiles green").

### W4. Transport & TLS surface + feature plumbing

```rust
#[non_exhaustive]
pub enum Transport { Tcp, Tls(TlsConfig), Ws, Wss(TlsConfig) }

#[non_exhaustive]
pub enum TlsConfig {
    Default,                      // backend/platform defaults
    Rustls(RustlsClientConfig),   // opaque newtype — see below
}

/// Opaque, ALWAYS present (enum shape never cfg-gated). The only way to
/// construct a non-trivial one is feature-gated:
pub struct RustlsClientConfig(/* private */);
#[cfg(feature = "tls-rustls")]
impl From<Arc<rustls::ClientConfig>> for RustlsClientConfig { .. }
```

- **Feature plumbing (new work item, was the critic's BLOCKER):** core has NO
  TLS features today (core/Cargo.toml:36 enables only `rumqttc/url`; root
  features forward straight to rumqttc, bypassing core — so core's private
  conversion couldn't even name `rumqttc::Transport::Tls`). Fix: core grows
  `tls-rustls`, `tls-rustls-no-provider`, `websocket`, `tls-native`(later),
  `proxy` features forwarding to the matching `rumqttc/*` features; root
  features re-route THROUGH core's features. Root's old `rumqttc-*` names
  stay one release as deprecated forwarding aliases (documented in
  CHANGELOG). `rumqttc-url` is dropped outright (not renamed): core already
  hard-enables `rumqttc/url`, and after W3 our own parser makes it moot.
- URL scheme sets `Tcp/Tls(Default)/Ws/Wss(Default)` at parse time; if the
  needed feature isn't compiled in, `connect` returns a clean
  "`TLS`/`websocket` support not compiled in — enable feature X" error
  (parse never fails on scheme availability).
- **rustls version coupling — documented semver-coupled exception:**
  `rustls`'s major version tracks the backend's `tokio-rustls` stack (0.26 /
  rustls 0.23 on rumqttc 0.25.1). The opaque newtype keeps `Transport`/
  `TlsConfig` shape stable across backend bumps; only the feature-gated
  `From` impl (and the `rustls` re-export) move. **Before locking this
  package: check rumqttc-v4-next's rustls version** (PLAN_0.3 §6 swap may
  land in the same release).
- L9 narrows: re-export `rustls` (users must build `ClientConfig`); drop the
  blanket `tokio_rustls` re-export (example 004 imports only
  `rustls::{ClientConfig, RootCertStore}`).
- Example 004 migrates: `opts.transport = Transport::Tls(rustls_cfg.into())`.

### W5. Re-export & prelude cleanup + docs

- core/src/lib.rs:153-159: remove `pub use rumqttc::{MqttOptions, QoS, Transport}`;
  export our `QoS`, `ConnectionOptions`, `Credentials`, `SessionPolicy`,
  `ProtocolVersion`, `Transport`, `TlsConfig`, new error types. Same for both
  preludes (core + root).
- Update remaining examples, doc-tests (incl. config.rs:74 doc wording),
  README snippets, COMPARISON doc.
- CHANGELOG: full migration table (old rumqttc-typed surface → new facade),
  one entry per leak L1-L13 + one row per dropped URL query param + feature
  renames.

## Acceptance criteria

1. `rg 'rumqttc' core/src --type rust` shows rumqttc only in: private
   conversion/event-loop code, the `unstable-backend-api` hatch
   (`BackendOptions`, `backend::rumqttc` re-export), and Cargo feature wiring.
   Zero hits in non-exempt public signatures/fields/re-exports.
2. `rg 'rumqttc' examples/` → zero hits (except a possible
   `unstable-backend-api` demo).
3. All examples compile & run against the dev broker; doc-tests pass.
4. Conversion-validation tests: sub-second keep-alive, `Resume` + empty
   client_id, `ResumeFor` on v4, `ResumeFor` overflow, `?protocol=5`,
   dropped-URL-param error, TLS-scheme-without-feature error.
5. Public API diff reviewed once against DUAL_PROTOCOL_API_DESIGN_2026.md's
   locked list (items 1, 4, 5, 6, 7 satisfied by W1-W5; items 2, 3 are
   PLAN_0.3 §2/§3, not this step).

## Resolved during critic pass (2026-07-09)

- TLS: opaque `RustlsClientConfig` newtype + core-level feature plumbing
  (shape never cfg-gated; BLOCKER fix).
- URL compat: deliberately NOT a superset — unmirrored params error with a
  pointer to the escape hatch.
- Facade validates instead of inheriting rumqttc's assert-panics.
- Escape hatch takes `BackendOptions<'_>` (non_exhaustive enum), not raw
  `MqttOptions` — 0.4-proof inside the exempt zone; hatch feature re-exports
  the backend crate.
- `max_packet_size` cut (YAGNI; dodges rumqttc's paired in/out params);
  `ClientOperationError::Backend` cut (dead in 0.3).
- Engine `QoS` needs no changes/release — derive parity already holds.

## Open questions (for Artem)

- Naming: `ConnectionOptions` vs `BrokerOptions` vs `MqttConnectOptions`?
  (Avoid `MqttOptions` verbatim — collides with rumqttc's in docs/search.)
- `Credentials.password: String` — plain for now, zeroize-style wrapper only
  if asked?
