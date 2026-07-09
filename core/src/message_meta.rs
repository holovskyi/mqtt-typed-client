//! Per-message MQTT metadata surfaced to subscribers.

use mqtt_topic_engine::QoS;

/// Protocol metadata attached to an incoming MQTT message.
///
/// Delivered alongside every message (as `Arc<MessageMeta>`, shared across all
/// subscribers of one publish). Read its fields; you never construct it in the
/// receive path — but [`MessageMeta::new`] is provided so you can build one in
/// your own handler tests.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MessageMeta {
	/// QoS of the delivered PUBLISH *packet* — NOT the subscription's granted
	/// QoS. With overlapping filters on one client the broker sends a single
	/// packet at the highest matching granted QoS, so a QoS-0 subscriber can
	/// observe a higher value here.
	pub qos: QoS,
	/// The broker delivered this as a retained message.
	pub retain: bool,
	/// The `dup` flag was set (a redelivery of a QoS > 0 packet).
	pub dup: bool,
	/// MQTT 5 properties. Always `None` on MQTT 3.1.1 (the only wire protocol in
	/// 0.3); reserved so v5 support lands additively.
	pub v5: Option<Mqtt5Meta>,
}

impl MessageMeta {
	/// Build metadata for an MQTT 3.1.1 message (`v5 == None`).
	///
	/// The library uses this on the receive path; it is public so downstream
	/// code can construct a `MessageMeta` in tests (the type is
	/// `#[non_exhaustive]`, so a struct literal is not available off-crate).
	pub fn new(qos: QoS, retain: bool, dup: bool) -> Self {
		Self {
			qos,
			retain,
			dup,
			v5: None,
		}
	}
}

/// MQTT 5 message properties.
///
/// Empty in 0.3 (a `MessageMeta.v5` is always `None`); fields (user properties,
/// content type, correlation data, response topic, message expiry) land with the
/// MQTT 5 backend. Reserved now so the visible shape is stable.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Mqtt5Meta {}

/// Raw per-message metadata carried from the event loop to the routing actor,
/// before it is promoted to a shared `Arc<MessageMeta>` in `handle_send`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawMeta {
	pub qos: QoS,
	pub retain: bool,
	pub dup: bool,
}
