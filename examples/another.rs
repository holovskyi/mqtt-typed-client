use std::sync::Arc;

use mqtt_typed_client::topic::topic_match::TopicMatch;
use mqtt_typed_client_macros::mqtt_topic;
/* payload concrete type */
#[derive(Debug, PartialEq)]
struct SensorData {
	timestamp: u64,
	measure_id: u32,
	value: f32,
}

/* MQTT message with all data */
#[allow(dead_code)]
#[mqtt_topic("typed/{room}/{sensor_id}/some/+/{temp}")]
#[derive(Debug)]
struct SensorReading {
	sensor_id: u32, // extracted from topic field. Other fiellds not allowed here
	room: String,
	temp: f32,

	payload: SensorData,    // optional filed
	topic: Arc<TopicMatch>, // optional filed
}

fn main() {}
