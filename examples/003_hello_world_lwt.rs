//! # Hello World with Last Will & Testament (LWT)
//!
//! This example demonstrates MQTT Last Will & Testament functionality:
//! - LWT messages are sent by broker when client disconnects unexpectedly
//! - Two separate connections: subscriber and publisher
//! - Publisher sets LWT, sends greeting, then crashes (simulated with exit)
//! - Subscriber receives both greeting and LWT messages
//!
//! Usage:
//! ```bash
//! # Terminal 1: Start subscriber (waits for messages)
//! cargo run --example 003_hello_world_lwt
//!
//! # Terminal 2: Run publisher (sends message then crashes)
//! cargo run --example 003_hello_world_lwt -- --publisher
//! ```
//!
//! Topic pattern: "greetings/{language}/{sender}"
//! LWT demonstrates ungraceful disconnect handling in MQTT

mod shared;

use std::env;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientConfig, QoS};
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
	// === 0. INITIALIZATION ===
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	// Parse command line arguments to determine mode
	// Default: subscriber, --publisher flag: publisher
	let args: Vec<String> = env::args().collect();
	let is_publisher = args.len() > 1 && args[1] == "--publisher";

	if is_publisher {
		run_publisher().await
	} else {
		run_subscriber().await
	}
}

/// Run subscriber that waits for greeting and LWT messages
async fn run_subscriber() -> Result<(), Box<dyn std::error::Error>> {
	println!("Starting MQTT Subscriber for LWT demo...");
	println!("\nIn another terminal, run:");
	println!("   cargo run --example 003_hello_world_lwt -- --publisher\n");

	// === 1. SUBSCRIBER CONNECTION ===
	// Connect to MQTT broker with unique client_id for subscriber
	// This client will listen for both normal messages and LWT messages
	let connection_url = shared::config::build_url("lwt_subscriber");
	println!("Connecting subscriber to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	println!("Subscriber connected to MQTT broker");

	// === 2. TOPIC SUBSCRIPTION ===
	// Get typed topic client for GreetingTopic structure
	// Subscribe to all greetings from any language and sender using wildcards
	let topic_client = client.greeting_topic();
	let mut subscriber = topic_client.subscribe().await?;

	println!(
		"Subscribed to: greetings/+/+ (will receive both normal and LWT \
		 messages)"
	);

	// === 3. RECEIVING MESSAGES ===
	// Wait for messages from the publisher
	// We expect: 1) normal greeting, 2) LWT when publisher crashes
	println!("Waiting for messages... (Press Ctrl+C to exit)\n");

	tokio::select! {
			_ = tokio::signal::ctrl_c() => {
				println!("\nSubscriber shutting down...");
			},
		_ = async {
			let mut message_count = 0;
			while let Some(result) = subscriber.receive().await {
				match result {
					Ok(greeting) => {
					message_count += 1;

					// Distinguish between normal messages and LWT messages
					if greeting.payload.text.contains("LWT") {
					println!("[{}] LWT from {}/{}: {} (publisher disconnected unexpectedly)",
					message_count, greeting.language, greeting.sender, greeting.payload.text);
					} else {
					println!("[{}] Greeting from {}/{}: {}",
					message_count, greeting.language, greeting.sender, greeting.payload.text);
					}
					},
					Err(e) => {
						eprintln!("Error receiving message: {e}");
					}
				}
			}
		} => {}
	};

	// === 4. CLEANUP ===
	// Gracefully shutdown the subscriber connection
	connection.shutdown().await?;
	println!("Subscriber disconnected gracefully\n");
	Ok(())
}

/// Run publisher that sets LWT, sends greeting, then crashes
async fn run_publisher() -> Result<(), Box<dyn std::error::Error>> {
	println!("Starting MQTT Publisher for LWT demo...");

	// === 1. PUBLISHER CONNECTION WITH LWT ===
	// Connect to MQTT broker with Last Will & Testament configured
	// LWT will be published automatically if this client disconnects unexpectedly
	let connection_url = shared::config::build_url("lwt_publisher");
	println!("Connecting publisher to MQTT broker: {connection_url}");

	// Create configuration with Last Will & Testament
	let mut config =
		MqttClientConfig::<BincodeSerializer>::from_url(&connection_url)?;

	// Create LWT message that will be sent if we disconnect unexpectedly
	let lwt_message = Message {
		text: "Bye bye LWT!".to_string(),
	};

	// Set up Last Will & Testament using the typed topic
	// This message will be published to greetings/rust/publisher if we crash
	let last_will = GreetingTopic::last_will("rust", "publisher", lwt_message)
		.qos(QoS::AtLeastOnce);

	config.with_last_will(last_will)?;

	println!(
		"LWT configured: 'Bye bye LWT!' on topic greetings/rust/publisher"
	);

	// Connect with LWT configuration
	let (client, _connection) = MqttClient::connect_with_config(config)
		.await
		.inspect_err(|e| {
		shared::config::print_connection_error(&connection_url, e);
	})?;

	println!("Publisher connected with LWT configured");

	// === 2. SEND NORMAL GREETING ===
	// First send a normal greeting message to show normal operation
	let topic_client = client.greeting_topic();

	let hello_message = Message {
		text: "Hello, World!".to_string(),
	};

	println!("Publishing greeting message...");
	topic_client
		.publish("rust", "publisher", &hello_message)
		.await?;

	println!("Greeting sent successfully!");

	// === 3. SIMULATE UNEXPECTED DISCONNECT ===
	// Crash the publisher without graceful shutdown to trigger LWT
	// In real scenarios, this could be power loss, network failure, etc.
	println!("\nSimulating unexpected disconnect in 2 seconds...");
	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

	println!("Publisher crashing now! (LWT should be triggered)");

	// This simulates an unexpected disconnect - the LWT will be published by broker
	// Note: std::process::exit() prevents graceful DISCONNECT message
	std::process::exit(0);
}
