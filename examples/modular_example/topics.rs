use std::sync::Arc;

use bincode::{Decode, Encode};
use mqtt_typed_client::topic::topic_match::TopicMatch;
use mqtt_typed_client_macros::mqtt_topic;

#[derive(Debug, Clone, Decode, Encode)]
pub struct TemperatureReading {
	pub device_id: usize,
	pub temperature: f32,
	pub humidity: Option<f32>,
	pub battery_level: Option<u8>,
}

#[derive(Debug)]
#[mqtt_topic("sensors/{location}/{sensor_type}/{device_id}/data")]
pub struct TemperatureTopic {
	pub location: String,
	pub sensor_type: String, // "temperature"
	pub device_id: usize,

	pub payload: TemperatureReading,
	pub topic: Arc<TopicMatch>,
}
