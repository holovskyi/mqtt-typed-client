//! Example showing generated typed client API

use mqtt_typed_client::{BincodeSerializer, MqttClient, QoS};
use mqtt_typed_client_macros::mqtt_topic;

#[derive(Debug)]
#[mqtt_topic("sensors/{sensor_id}/temperature")]
struct SensorReading {
	sensor_id: u32,
	payload: f64,
}

#[tokio::main]
async fn main() {
	println!("=== Typed Client Demo ===");

	// Show the generated constants
	println!("Topic pattern: {}", SensorReading::TOPIC_PATTERN);
	println!("MQTT pattern:  {}", SensorReading::MQTT_PATTERN);

	let mut config =
		mqtt_typed_client::MqttClientConfig::<BincodeSerializer>::new(
			"test_client",
			"broker.mqtt.cool",
			1883,
		);

	let last_will = SensorReading::last_will(10, 0.0).qos(QoS::AtLeastOnce);

	config.with_last_will(last_will).unwrap();

	let (client, connection) =
		MqttClient::connect_with_config(config).await.unwrap();

	// // Using the extension trait method:
	let sensor_client = client.sensor_reading();

	// // Type-safe publishing:
	sensor_client.publish(123, &25.5).await.unwrap();

	// Get publisher for reuse:
	let publisher = sensor_client.get_publisher(123).unwrap();
	publisher.publish(&26.0).await.unwrap();

	// Subscribe to sensor data:
	let mut subscriber = sensor_client.subscribe().await.unwrap();
	while let Some(result) = subscriber.receive().await {
		match result {
			| Ok(reading) => {
				println!("Sensor {}: {}°C", reading.sensor_id, reading.payload)
			}
			| Err(e) => eprintln!("Parse error: {e:?}"),
		}
	}

	connection.shutdown().await.unwrap();

	println!("Check the macro expansion to see generated typed client code!");
}
