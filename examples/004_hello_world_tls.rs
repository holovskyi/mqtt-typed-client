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
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientConfig};
use mqtt_typed_client_macros::mqtt_topic;
use rumqttc::Transport;

use std::{fs, io::BufReader};
use rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

/// Create TLS configuration with custom CA certificate
fn create_tls_config() -> Result<ClientConfig, Box<dyn std::error::Error>> {
	let mut root_cert_store = RootCertStore::empty();
	
	// Load CA certificate
	let ca_cert = fs::read("dev/certs/ca.pem")?;
	let mut reader = BufReader::new(&ca_cert[..]);
	
	// Parse PEM certificates
	let certs = rustls_pemfile::certs(&mut reader);
	for cert in certs {
		let cert = cert?;
		root_cert_store.add(cert)?;
	}
	
	let config = ClientConfig::builder()
		.with_root_certificates(root_cert_store)
		.with_no_client_auth();
	
	Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
		// Initialize logging
	tracing_subscriber::registry()
	    .with(
	        tracing_subscriber::EnvFilter::try_from_default_env()
	            .unwrap_or_else(|_| "debug".into()),
	    )
	    .with(
	        tracing_subscriber::fmt::layer()
	            .with_target(true)
	            .with_thread_ids(false)
	            .with_thread_names(false)
	            .with_file(false)
	            .with_line_number(false)
	            .compact(),
	    )
	    .init();
	println!("Starting MQTT Hello World example...\n");

	// === 1. CONNECTION ===
	// Create TLS configuration with custom CA certificate
	let tls_config = create_tls_config()?;
	
	// Configure MQTT client with TLS
	let mut config = MqttClientConfig::<BincodeSerializer>::new(
		"hello_world_tls_client",
		"localhost",
		8883,
	);
	
	// Set TLS transport
	config.connection.set_transport(Transport::tls_with_config(tls_config.into()));
	
	// Connect to MQTT broker using custom configuration
	let (client, connection) = MqttClient::connect_with_config(config)
		.await
		.inspect_err(|e| {
			eprintln!("Connection failed: {e}");
			eprintln!(
				"Ensure EMQX is running with TLS on localhost:8883 and CA cert exists in emqx-certs/ca.pem"
			);
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
