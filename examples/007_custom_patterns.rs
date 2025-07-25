//! # Custom Topic Patterns - MQTT Typed Client
//!
//! This example demonstrates how to override default topic patterns while
//! maintaining type safety and parameter compatibility.
//!
//! The `#[mqtt_topic]` macro generates a default pattern, but you can use
//! custom patterns for different environments, multi-tenant systems, or
//! legacy compatibility.
//!
//! ## Key APIs demonstrated:
//!
//! - **Custom subscription**: `.with_pattern()` instead of `.subscribe()`
//! - **Custom publishing**: `.get_publisher_to()` instead of `.publish()`
//! - **Custom Last Will**: `.last_will_to()` instead of `.last_will()`
//!
//! ## Pattern Compatibility Rules:
//!
//! Custom patterns must have identical wildcard structure:
//! - Same number of parameters: `{language}`, `{sender}`
//! - Same parameter names and order
//! - Same wildcard types (`+` vs `#`)
//!
//! Compatible: "greetings/{language}/{sender}" → "dev/greetings/{language}/{sender}"
//! Incompatible: "greetings/{language}/{sender}" → "greetings/{sender}/{language}"

mod shared;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, QoS};
use mqtt_typed_client_macros::mqtt_topic;

/// Message payload - automatically serialized/deserialized with bincode
#[derive(Encode, Decode, Debug, Clone)]
struct Message {
	text: String,
}

/// Topic structure with automatic parameter extraction from MQTT topic path
///
/// **Default pattern**: "greetings/{language}/{sender}"
/// **Default MQTT pattern**: "greetings/+/+" (for broker subscription)
///
/// We'll demonstrate using custom patterns like:
/// - "dev/greetings/{language}/{sender}" (environment prefix)
/// - "tenant_123/greetings/{language}/{sender}" (multi-tenant)
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

	println!("Starting Custom Topic Patterns example...\n");

	// === 1. CONNECTION ===
	let connection_url = shared::config::build_url("custom_patterns");
	println!("Connecting to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	println!("Connected to MQTT broker");

	// Get typed topic client for GreetingTopic structure
	let topic_client = client.greeting_topic();

	println!("Default pattern: {}", GreetingTopic::TOPIC_PATTERN);
	println!("Default MQTT pattern: {}\n", GreetingTopic::MQTT_PATTERN);

	// === 2. CUSTOM SUBSCRIPTION WITH .with_pattern() ===
	// Instead of: topic_client.subscribe().await?
	// Use: topic_client.subscription().with_pattern().subscribe().await?

	println!("Setting up custom subscription with environment prefix...");

	let mut custom_subscriber = topic_client
		.subscription()
		.with_pattern("dev/greetings/{language}/{sender}")? // Compatible: same {language}, {sender}
		.subscribe()
		.await?;

	println!("Subscribed to: dev/greetings/+/+");

	// Small delay to ensure subscription is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// === 3. CUSTOM PUBLISHING WITH .get_publisher_to() ===
	// Instead of: topic_client.publish("rust", "alice", &message).await?
	// Use: topic_client.get_publisher_to().publish().await?

	let message = Message {
		text: "Hello from custom pattern!".to_string(),
	};

	println!("Publishing to custom pattern...");

	let custom_publisher = topic_client.get_publisher_to(
		"dev/greetings/{language}/{sender}", // Compatible custom pattern
		"rust",                              // {language} parameter
		"dev_user",                          // {sender} parameter
	)?;

	custom_publisher.publish(&message).await?;
	println!("Published to: dev/greetings/rust/dev_user");

	// === 4. CUSTOM LAST WILL WITH .last_will_to() ===
	// Instead of: GreetingTopic::last_will("rust", "client", lwt_message)
	// Use: GreetingTopic::last_will_to() with custom pattern

	let lwt_message = Message {
		text: "Client disconnected unexpectedly!".to_string(),
	};

	let custom_lwt = GreetingTopic::last_will_to(
		"dev/greetings/{language}/{sender}", // Compatible custom pattern
		"rust",
		"dev_client",
		lwt_message,
	)?
	.qos(QoS::AtLeastOnce);

	println!("Custom LWT topic: {}", custom_lwt.topic);

	// === 5. RECEIVING MESSAGES ===
	println!("\nWaiting for published message...");

	if let Some(Ok(greeting)) = custom_subscriber.receive().await {
		println!("Received custom greeting:");
		println!("   Language: {}", greeting.language);
		println!("   Sender: {}", greeting.sender);
		println!("   Message: {}", greeting.payload.text);
	}

	// === 6. CLEANUP ===
	connection.shutdown().await?;
	println!("\nGoodbye!");

	Ok(())
}
