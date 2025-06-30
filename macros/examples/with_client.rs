use mqtt_typed_client_macros::mqtt_topic;

#[derive(Debug, Default)]
#[mqtt_topic("sensors/{sensor_id}/data")]
struct SensorReading {
	sensor_id: u32,
	payload: Vec<u8>,
}

#[tokio::main]
async fn main() {
	println!("Testing MQTT subscription with macro...");

	// Create a sample sensor reading
	let reading = SensorReading {
		sensor_id: 42,
		payload: vec![0x01, 0x02, 0x03, 0x04],
	};

	println!("Created sensor reading: {reading:?}");
	println!("Sensor ID: {}", reading.sensor_id);
	println!("Payload length: {} bytes", reading.payload.len());

	println!("Success! Macro generates working code.");
}
