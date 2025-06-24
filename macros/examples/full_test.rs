use std::sync::Arc;

use mqtt_typed_client::client::async_client::MqttClient;
use mqtt_typed_client::message_serializer::BincodeSerializer;
use mqtt_typed_client_macros::mqtt_topic;

#[allow(dead_code)]
#[derive(Debug)]
#[mqtt_topic("sensors/{sensor_id}/temperature")]
struct TemperatureReading {
	sensor_id: u32,
	payload: Vec<u8>,
	topic: Arc<mqtt_typed_client::topic::topic_match::TopicMatch>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🚀 Testing full MQTT macro integration...");

	println!("📋 Topic pattern: {}", TemperatureReading::TOPIC_PATTERN);

	println!("🔌 Attempting MQTT connection...");

	match MqttClient::<BincodeSerializer>::connect(
		"mqtt://broker.mqtt.cool:1883?client_id=test_client",
	)
	.await
	{
		| Ok((client, _connection)) => {
			println!("✅ MQTT client created successfully!");

			match TemperatureReading::subscribe(&client).await {
				| Ok(_subscriber) => {
					println!(
						"🎉 Successfully subscribed using macro-generated \
						 method!"
					);
				}
				| Err(e) => {
					println!("❌ Subscription failed: {}", e);
				}
			}
		}
		| Err(e) => {
			println!(
				"❌ MQTT connection failed (expected without broker): {}",
				e
			);
			println!("✅ But macro compilation successful!");
		}
	}

	Ok(())
}
