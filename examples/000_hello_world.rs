//! # Hello World - MQTT Typed Client
//!
//! A simple example demonstrating key features:
//! - Automatic topic parameter parsing with `#[mqtt_topic]` macro
//! - Type-safe message routing
//! - Automatic serialization/deserialization
//!
//! Topic pattern: "greetings/{language}/{sender}"
//! Example: "greetings/rust/alice" → GreetingTopic { language: "rust", sender: "alice", payload: Message }

mod shared;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	println!("Starting MQTT Hello World example...\n");

	// === 1. CONNECTION ===
	// Connect to MQTT broker using BincodeSerializer for efficient binary serialization
	// URL and client_id are automatically configured from environment or defaults
	let connection_url = shared::config::build_url("hello_world");
	println!("Connecting to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	println!("Connected to MQTT broker");

	// === 2. TOPIC SUBSCRIPTION ===
	// Get typed topic client for GreetingTopic structure
	let topic_client = client.greeting_topic();

	// Subscribe to all greetings from any language and sender
	// MQTT pattern: "greetings/+/+" ('+' is wildcard for any single topic level)
	let mut subscriber = topic_client.subscribe().await?;

	println!("Subscribed to: greetings/+/+");

	// === 3. MESSAGE PUBLISHING ===
	// Small delay to ensure that subscription is ready
	// This is just for demonstration purposes because subscriber
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

	// === 4. RECEIVING MESSAGES ===
	println!("Waiting for greeting message from broker...");
	// Wait for the first received message (our own greeting in this case)
	if let Some(Ok(greeting)) = subscriber.receive().await {
		println!("Received greeting:");
		println!("   Language: {}", greeting.language);
		println!("   Sender: {}", greeting.sender);
		println!("   Message: {}", greeting.payload.text);
	}

	// === 5. CLEANUP ===
	// Gracefully shutdown the connection
	connection.shutdown().await?;
	println!("\nGoodbye!");

	Ok(())
}
