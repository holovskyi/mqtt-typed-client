//! Last Will and Testament message types

use rumqttc::QoS;

/// Represents a Last Will and Testament (LWT) message for MQTT clients.
pub struct TypedLastWill<T> {
	/// The payload of the LWT message.
	pub payload: T,
	/// The Quality of Service level for the LWT message.
	pub qos: QoS,
	/// Whether the LWT message should be retained by the broker.
	pub retain: bool,
	/// The topic to which the LWT message will be published.
	pub topic: String,
}

impl<T> TypedLastWill<T> {
	/// Creates a new Last Will and Testament message.
	pub fn new(topic: String, payload: T) -> Self {
		Self {
			payload,
			qos: QoS::AtLeastOnce,
			retain: false,
			topic,
		}
	}

	/// Sets the QoS level for the LWT message.
	pub fn qos(mut self, qos: QoS) -> Self {
		self.qos = qos;
		self
	}

	/// Sets the retain flag for the LWT message.
	pub fn retain(mut self, retain: bool) -> Self {
		self.retain = retain;
		self
	}
}
