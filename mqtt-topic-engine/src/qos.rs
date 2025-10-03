//! Quality of Service (QoS) levels for MQTT
//!
//! Defines the three standard MQTT QoS levels independent of any specific
//! MQTT client implementation.
//!
//! This module provides conversions to/from popular MQTT client libraries:
//! - `rumqttc` - Enable with feature `rumqttc`
//! - `paho-mqtt` - Enable with feature `paho-mqtt`

use std::fmt;

/// MQTT Quality of Service levels
///
/// Defines delivery guarantees for MQTT messages:
/// - `AtMostOnce` (0): Best effort delivery, no guarantees
/// - `AtLeastOnce` (1): Message delivered at least once, duplicates possible
/// - `ExactlyOnce` (2): Message delivered exactly once, highest guarantee
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum QoS {
	/// QoS 0: At most once delivery (fire and forget)
	AtMostOnce = 0,
	/// QoS 1: At least once delivery (acknowledged delivery)
	AtLeastOnce = 1,
	/// QoS 2: Exactly once delivery (assured delivery)
	ExactlyOnce = 2,
}

impl QoS {
	/// Convert to rumqttc QoS type
	///
	/// # Example
	/// ```ignore
	/// let qos = QoS::AtLeastOnce;
	/// let rumqttc_qos = qos.to_rumqttc();
	/// ```
	#[cfg(feature = "rumqttc")]
	pub fn to_rumqttc(self) -> rumqttc::QoS {
		match self {
			| QoS::AtMostOnce => rumqttc::QoS::AtMostOnce,
			| QoS::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
			| QoS::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
		}
	}

	/// Convert to paho-mqtt QoS type
	///
	/// # Example
	/// ```ignore
	/// let qos = QoS::AtLeastOnce;
	/// let paho_qos = qos.to_paho_mqtt();
	/// ```
	#[cfg(feature = "paho-mqtt")]
	pub fn to_paho_mqtt(self) -> paho_mqtt::QoS {
		match self {
			| QoS::AtMostOnce => paho_mqtt::QoS::AtMostOnce,
			| QoS::AtLeastOnce => paho_mqtt::QoS::AtLeastOnce,
			| QoS::ExactlyOnce => paho_mqtt::QoS::ExactlyOnce,
		}
	}
}

#[cfg(feature = "rumqttc")]
impl From<rumqttc::QoS> for QoS {
	fn from(qos: rumqttc::QoS) -> Self {
		match qos {
			| rumqttc::QoS::AtMostOnce => QoS::AtMostOnce,
			| rumqttc::QoS::AtLeastOnce => QoS::AtLeastOnce,
			| rumqttc::QoS::ExactlyOnce => QoS::ExactlyOnce,
		}
	}
}

#[cfg(feature = "paho-mqtt")]
impl From<paho_mqtt::QoS> for QoS {
	fn from(qos: paho_mqtt::QoS) -> Self {
		match qos {
			| paho_mqtt::QoS::AtMostOnce => QoS::AtMostOnce,
			| paho_mqtt::QoS::AtLeastOnce => QoS::AtLeastOnce,
			| paho_mqtt::QoS::ExactlyOnce => QoS::ExactlyOnce,
		}
	}
}

impl fmt::Display for QoS {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			| QoS::AtMostOnce => write!(f, "QoS0"),
			| QoS::AtLeastOnce => write!(f, "QoS1"),
			| QoS::ExactlyOnce => write!(f, "QoS2"),
		}
	}
}
