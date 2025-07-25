use std::time::Duration;

use mqtt_typed_client::prelude::*;
use tokio::select;

use super::topics::TemperatureReading;
// Import patterns demonstration:
// Variant A - Import everything (less explicit but convenient)
// use super::topics::*;

// Variant B - Specific imports (recommended for clarity and avoiding conflicts)
use super::topics::temperature_topic::*;
// Import shared utilities
use crate::shared;

pub async fn run_example() -> Result<()> {
	println!("Initializing multi-sensor monitoring system...\n");

	// === 1. CONNECTION ===
	// Connect to MQTT broker using BincodeSerializer for efficient binary serialization
	let connection_url = shared::config::build_url("modular_sensor_system");
	println!("Connecting to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	println!("- Connected to MQTT broker\n");

	// === 2. TOPIC SETUP ===
	// Get typed topic client for temperature sensor operations
	let temp_client = client.temperature_topic();

	// === 3. SAMPLE DATA ===
	// Create sample temperature reading from a floor sensor
	let temp_reading = TemperatureReading {
		device_id: 42,
		temperature: 23.5,
		humidity: Some(45.0),
		battery_level: Some(80),
	};

	println!("Sample sensor data: {temp_reading:?}\n");

	// === 4. PUBLISHER SETUP ===
	// Create publisher for: "sensors/Home/floor/37/data"
	let publisher = temp_client.get_publisher("Home", "floor", 37)?;

	// Spawn publisher task with proper error handling
	let publisher_handle = {
		let temp_data = temp_reading.clone();
		tokio::spawn(async move {
			// MQTT SUBACK timing: Give subscribers time to register
			// In production, use proper discovery patterns or SUBACK confirmation
			tokio::time::sleep(Duration::from_millis(500)).await;

			println!("Publishing sensor data to: sensors/Home/floor/37/data");
			if let Err(e) = publisher.publish(&temp_data).await {
				eprintln!("Publish error: {e}");
				return Err(e);
			}
			println!("- Data published successfully\n");
			Ok(())
		})
	};

	// === 5. SUBSCRIPTION SETUP ===
	// Subscriber A: Monitor ALL sensors (wildcard pattern: "sensors/+/+/+/data")
	println!("Setting up wildcard subscriber for all sensors...");
	let mut subscriber_all = temp_client.subscribe().await?;
	println!("- Subscribed to: sensors/+/+/+/data\n");

	// Subscriber B: Monitor specific device only (filtered pattern)
	println!("Setting up filtered subscriber for device 370...");
	let mut subscriber_specific = temp_client
		.subscription()
		.for_device_id(370)
		.with_cache(100)
		.subscribe()
		.await?;
	println!(
		"- Subscribed to: sensors/+/+/370/data (with 100-message cache)\n"
	);

	// === 6. MESSAGE PROCESSING ===
	println!("Listening for temperature messages...\n");

	// Set up timeout for graceful demo completion
	let timeout_duration = Duration::from_secs(10);
	let mut message_count = 0;
	let max_messages = 5;

	let monitoring_result = tokio::time::timeout(timeout_duration, async {
		loop {
			select! {
				// Handle messages from wildcard subscription
				msg_result = subscriber_all.receive() => {
					match msg_result {
						Some(Ok(temp_msg)) => {
							message_count += 1;
							println!("[ALL-SENSORS] Received from: {}", temp_msg.topic.topic_path());
							println!("   Location: {} | Sensor: {} | Device: {}", 
								temp_msg.location, temp_msg.sensor_type, temp_msg.device_id);
							println!("   Data: temp={}°C, humidity={:?}%, battery={:?}%\n", 
								temp_msg.payload.temperature,
								temp_msg.payload.humidity,
								temp_msg.payload.battery_level);

							if message_count >= max_messages {
								break;
							}
						},
						Some(Err(e)) => {
							eprintln!("Wildcard subscription error: {e}");
						},
						None => {
							println!("Wildcard subscription ended");
							break;
						}
					}
				},
				// Handle messages from specific device subscription
				msg_result = subscriber_specific.receive() => {
					match msg_result {
						Some(Ok(temp_msg)) => {
							println!("[DEVICE-370] Specific device message: {}", temp_msg.topic.topic_path());
							println!("   This would only trigger for device_id=370\n");
						},
						Some(Err(e)) => {
							eprintln!("Specific subscription error: {e}");
						},
						None => {
							println!("Specific subscription ended");
							break;
						}
					}
				}
			}
		}
	}).await;

	// === 7. CLEANUP ===
	// Wait for publisher to complete
	if let Err(e) = publisher_handle.await.unwrap_or(Ok(())) {
		eprintln!("Publisher task error: {e}");
	}

	match monitoring_result {
		| Ok(_) => println!("Processed {message_count} messages successfully"),
		| Err(_) => println!("Demo timeout reached ({timeout_duration:?})"),
	}

	// Graceful connection shutdown
	println!("\nShutting down connection...");
	connection.shutdown().await?;
	println!("- Connection closed gracefully");

	Ok(())
}
