//! # Hello World - MQTT Typed Client
//!
//! A simple example demonstrating key features:
//! - Automatic topic parameter parsing with `#[mqtt_topic]` macro
//! - Type-safe message routing
//! - Automatic serialization/deserialization
//!
//! Topic pattern: "greetings/{language}/{sender}"
//! Example: "greetings/rust/alice" → GreetingTopic { language: "rust", sender: "alice", payload: Message }

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient};
use mqtt_typed_client_macros::mqtt_topic;

/// Message payload - automatically serialized/deserialized with bincode
#[derive(Encode, Decode, Debug)]
struct Message {
	text: String,
}

/// Topic structure with automatic parameter extraction from MQTT topic path
///
/// Pattern: "greetings/{language}/{sender}"
/// - Subscription: "greetings/+/+" (subscribes to all greetings regardless of language and sender)
/// - Publishing: client.publish("rust", "alice", &msg) → "greetings/rust/alice"
/// - Receiving: "greetings/rust/alice" → GreetingTopic { language: "rust", sender: "alice", payload: deserialized_msg }
#[mqtt_topic("greetings/{language}/{sender}")]
pub struct GreetingTopic {
	language: String, // Extracted from first topic parameter {language}
	sender: String,   // Extracted from second topic parameter {sender}
	payload: Message, // Automatically deserialized message payload
}

/// Get MQTT broker URL from environment or use default public broker
fn broker_url() -> String {
	std::env::var("MQTT_BROKER").unwrap_or_else(|_| {
		"mqtt://broker.hivemq.com:1883?client_id=hello_world_example".to_string()
		//You can try other free mqtt broker
		//"mqtt://broker.mqtt.cool:1883?client_id=test_client_example".to_string()
	})
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("Starting MQTT Hello World example...\n");

	// Connect to MQTT broker using BincodeSerializer for efficient binary serialization
	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&broker_url())
			.await
			.inspect_err(|e| {
				eprintln!("Connection failed: {e}");
				eprintln!(
					"Try: MQTT_BROKER=\"mqtt://localhost:1883\" cargo run \
					 --example 000_hello_world"
				);
			})?;

	println!("Connected to MQTT broker");

	// Get typed topic client for GreetingTopic structure
	let topic_client = client.greeting_topic();

	// Subscribe to all greetings from any language and sender
	// MQTT pattern: "greetings/+/+" ('+' is wildcard for any single topic level)
	let mut subscriber = topic_client.subscribe().await?;

	println!("Subscribed to: greetings/+/+");

    // Small delay to ensure that subscription is ready
    // This is just for demonstration purposes because subsciber 
	// and publisher are in the same process.
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Create message
	let hello_message = Message {
		text: "Hello, World!".to_string(),
	};

	println!("Publishing greeting message to topic: greetings/rust/rustacean");


	// Publish message to topic "greetings/rust/rustacean"
	// Parameters are automatically inserted into topic pattern
	topic_client
		.publish("rust", "rustacean", &hello_message)
		.await?;


	println!("Waiting for greeting message from broker...");
	// Wait for the first received message (our own greeting in this case)
	if let Some(Ok(greeting)) = subscriber.receive().await {
		println!("Received greeting:");
		println!("   Language: {}", greeting.language);
		println!("   Sender: {}", greeting.sender);
		println!("   Message: {}", greeting.payload.text);
	}

	// Gracefully shutdown the connection
	connection.shutdown().await?;
	println!("\nGoodbye!");

	Ok(())
}
