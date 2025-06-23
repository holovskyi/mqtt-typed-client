use mqtt_typed_client_macros::mqtt_topic;

#[derive(Debug, Default)]
#[mqtt_topic("sensors/{sensor_id}/+/+/data/{room}")]
struct SensorReading {
	sensor_id: u32,
	room: String,
	payload: String,
}

fn main() {
	println!("Topic pattern: {}", SensorReading::TOPIC_PATTERN);
	println!("MQTT pattern: {}", SensorReading::MQTT_PATTERN);
	let reading = SensorReading {
		sensor_id: 1,
		room: "Living Room".to_string(),
		payload: "Temperature: 22C".to_string(),
	};
}
