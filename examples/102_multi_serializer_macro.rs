//! # Multi-Serializer with Macro Support
//!
//! Demonstrates using different serializers for different topics
//! via the #[mqtt_topic] macro with serializer attribute.
//!
//! This shows the high-level API where serializer is part of the
//! message type definition.

mod shared;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, JsonSerializer, MqttClient};
use mqtt_typed_client_macros::mqtt_topic;
use serde::{Deserialize, Serialize};

/// Modern binary data using Bincode (default from client)
#[derive(Encode, Decode, Debug)]
struct BinaryData {
	value: u64,
	timestamp: u64,
}

/// Modern topic using default serializer (Bincode from client)
#[mqtt_topic("modern/sensors/{sensor_id}/data")]
struct ModernSensor {
	sensor_id: u32,
	payload: BinaryData,
}

/// Legacy JSON data for backward compatibility
#[derive(Serialize, Deserialize, Debug)]
struct LegacyData {
	message: String,
	count: i32,
}

/// Legacy topic explicitly using JSON serializer
#[mqtt_topic("legacy/devices/{device_id}/status", serializer = JsonSerializer)]
struct LegacyDevice {
	device_id: String,
	payload: LegacyData,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	shared::tracing::setup(None);

	println!("Testing multi-serializer with macro support...\n");

	// Connect with Bincode as default serializer
	let connection_url = shared::config::build_url("multi_serializer_macro");
	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	println!("✓ Connected with BincodeSerializer as default");

	// Subscribe to modern sensor (uses default Bincode)
	let mut modern_sub = ModernSensor::subscribe(&client).await?;
	println!("✓ Subscribed to 'modern/sensors/+/data' with BincodeSerializer");

	// Subscribe to legacy device (uses JSON via macro)
	let mut legacy_sub = LegacyDevice::subscribe(&client).await?;
	println!(
		"✓ Subscribed to 'legacy/devices/+/status' with JsonSerializer (from \
		 macro)"
	);

	// Small delay to ensure subscriptions are ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Publish modern sensor data (Bincode)
	let modern_data = BinaryData {
		value: 42,
		timestamp: 1234567890,
	};
	ModernSensor::publish(&client, 101, &modern_data).await?;
	println!("✓ Published modern sensor data with BincodeSerializer");

	// Publish legacy device data (JSON via macro)
	let legacy_data = LegacyData {
		message: "System operational".to_string(),
		count: 5,
	};
	LegacyDevice::publish(&client, "device-abc", &legacy_data).await?;
	println!("✓ Published legacy device data with JsonSerializer (from macro)");

	// Receive messages
	println!("\nWaiting for messages...");

	tokio::select! {
		Some(result) = modern_sub.receive() => {
			match result {
				Ok(msg) => {
					println!("✓ Received modern sensor {}: value={}, timestamp={}",
						msg.sensor_id, msg.payload.value, msg.payload.timestamp);
				}
				Err(e) => println!("✗ Failed to deserialize modern data: {:?}", e),
			}
		}
		Some(result) = legacy_sub.receive() => {
			match result {
				Ok(msg) => {
					println!("✓ Received legacy device {}: message='{}', count={}",
						msg.device_id, msg.payload.message, msg.payload.count);
				}
				Err(e) => println!("✗ Failed to deserialize legacy data: {:?}", e),
			}
		}
	}

	// Wait for second message
	tokio::select! {
		Some(result) = modern_sub.receive() => {
			match result {
				Ok(msg) => {
					println!("✓ Received modern sensor {}: value={}, timestamp={}",
						msg.sensor_id, msg.payload.value, msg.payload.timestamp);
				}
				Err(e) => println!("✗ Failed to deserialize modern data: {:?}", e),
			}
		}
		Some(result) = legacy_sub.receive() => {
			match result {
				Ok(msg) => {
					println!("✓ Received legacy device {}: message='{}', count={}",
						msg.device_id, msg.payload.message, msg.payload.count);
				}
				Err(e) => println!("✗ Failed to deserialize legacy data: {:?}", e),
			}
		}
	}

	connection.shutdown().await?;
	println!("\n✓ Macro-based multi-serializer test completed successfully!");

	Ok(())
}
