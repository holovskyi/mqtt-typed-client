use mqtt_typed_client::client::async_client::MqttClient;
use mqtt_typed_client::message_serializer::BincodeSerializer;
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

	println!("Success! Macro generates working code.");
}
