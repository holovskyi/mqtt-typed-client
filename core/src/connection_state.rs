//! Observable connection lifecycle state.
//!
//! Published on a [`tokio::sync::watch`] channel and read via
//! [`MqttClient::connection_state`](crate::MqttClient::connection_state). The
//! channel is level-triggered: consumers observe the latest state, and rapid
//! intermediate transitions (e.g. `Reconnecting{1}` → `Reconnecting{2}`) may
//! collapse. The terminal [`ConnectionState::Disconnected`] is never missed —
//! `watch` always retains the last value.

/// The connection's current lifecycle state.
///
/// Designed v5-first: every variant is `#[non_exhaustive]` so protocol-5 fields
/// (reason codes, properties) can be added without a breaking change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
	/// The connection is up. `session_present` reports whether the broker
	/// resumed an existing session (v4 CONNACK today; v5 CONNACK identically
	/// later) — `false` means a clean session, so subscriptions were
	/// re-established by the client.
	#[non_exhaustive]
	Connected {
		/// Broker resumed a prior session rather than starting a clean one.
		session_present: bool,
	},

	/// The connection was lost and the backend is retrying. `attempt` is the
	/// number of *consecutive poll failures since the last successful poll*; it
	/// resets to 0 on any success (a reconnect, a delivered message, or another
	/// notification). It is tracked separately from the internal error counter
	/// that drives backoff and [`DisconnectReason::MaxErrorsExceeded`], so a
	/// broker that keeps reconnecting is not permanently killed yet a truly
	/// unreachable broker still terminates.
	#[non_exhaustive]
	Reconnecting {
		/// Consecutive poll failures since the last successful poll.
		attempt: u32,
	},

	/// Terminal. The event loop has exited; no further transitions occur and
	/// every subscriber's `receive()` now yields `None`.
	#[non_exhaustive]
	Disconnected {
		/// Why the event loop terminated.
		reason: DisconnectReason,
	},
}

/// Why the connection reached its terminal [`ConnectionState::Disconnected`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
	/// [`MqttConnection::shutdown`](crate::MqttConnection::shutdown) was called
	/// (the outgoing-DISCONNECT path).
	CleanShutdown,

	/// The broker sent a DISCONNECT (the incoming-DISCONNECT path). The v5
	/// disconnect reason code arrives here additively in 0.4.
	#[non_exhaustive]
	BrokerDisconnected {},

	/// The event loop terminated after too many consecutive poll errors.
	#[non_exhaustive]
	MaxErrorsExceeded {
		/// The consecutive-error count at termination.
		errors: u32,
	},
}
