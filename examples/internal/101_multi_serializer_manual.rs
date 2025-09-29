//! Test example for multi-serializer support
//!
//! Demonstrates using different serializers for different topics
//! within a single MQTT session.

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, JsonSerializer, MqttClient};
use serde::{Deserialize, Serialize};

/// Binary data using Bincode
#[derive(Encode, Decode, Debug)]
struct BinaryData {
	value: u64,
	timestamp: u64,
}

/// JSON data for legacy compatibility
#[derive(Serialize, Deserialize, Debug)]
struct JsonData {
	message: String,
	count: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("Testing multi-serializer support...\n");

	// Connect with Bincode as default serializer
	let url = "mqtt://localhost:1883?client_id=multi_serializer_test";
	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(url).await?;

	println!("✓ Connected with BincodeSerializer as default");

	// Clone client with JSON serializer for legacy topics
	let json_client = client.clone_with_serializer::<JsonSerializer>();
	println!("✓ Cloned client with JsonSerializer");

	// Create subscribers with different serializers
	let mut bincode_sub = client.subscribe::<BinaryData>("binary/data").await?;
	println!("✓ Subscribed to 'binary/data' with BincodeSerializer");

	let mut json_sub = json_client.subscribe::<JsonData>("json/data").await?;
	println!("✓ Subscribed to 'json/data' with JsonSerializer");

	// Small delay to ensure subscriptions are ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Publish binary data with Bincode
	let binary_pub = client.get_publisher::<BinaryData>("binary/data")?;
	let binary_msg = BinaryData {
		value: 42,
		timestamp: 1234567890,
	};
	binary_pub.publish(&binary_msg).await?;
	println!("✓ Published binary data with BincodeSerializer");

	// Publish JSON data
	let json_pub = json_client.get_publisher::<JsonData>("json/data")?;
	let json_msg = JsonData {
		message: "Hello from JSON".to_string(),
		count: 10,
	};
	json_pub.publish(&json_msg).await?;
	println!("✓ Published JSON data with JsonSerializer");

	// Receive messages
	println!("\nWaiting for messages...");

	tokio::select! {
		Some((topic, result)) = bincode_sub.receive() => {
			match result {
				Ok(data) => {
					println!("✓ Received binary data from {}: value={}, timestamp={}",
						topic.topic_path(), data.value, data.timestamp);
				}
				Err(e) => println!("✗ Failed to deserialize binary data: {e:?}"),
			}
		}
		Some((topic, result)) = json_sub.receive() => {
			match result {
				Ok(data) => {
					println!("✓ Received JSON data from {}: message='{}', count={}",
						topic.topic_path(), data.message, data.count);
				}
				Err(e) => println!("✗ Failed to deserialize JSON data: {e:?}"),
			}
		}
	}

	// Wait for second message
	tokio::select! {
		Some((topic, result)) = bincode_sub.receive() => {
			match result {
				Ok(data) => {
					println!("✓ Received binary data from {}: value={}, timestamp={}",
						topic.topic_path(), data.value, data.timestamp);
				}
				Err(e) => println!("✗ Failed to deserialize binary data: {e:?}"),
			}
		}
		Some((topic, result)) = json_sub.receive() => {
			match result {
				Ok(data) => {
					println!("✓ Received JSON data from {}: message='{}', count={}",
						topic.topic_path(), data.message, data.count);
				}
				Err(e) => println!("✗ Failed to deserialize JSON data: {e:?}"),
			}
		}
	}

	connection.shutdown().await?;
	println!("\n✓ Test completed successfully!");

	Ok(())
}
