//! # MQTT Retain & Clear Messages Demo
//!
//! Demonstrates MQTT retained message functionality with multiple clients connecting
//! at different times to showcase how retained messages work in real-world scenarios.
//!
//! ## Key MQTT Concepts Demonstrated:
//!
//! **Retained Messages:**
//! - Broker stores the last retained message for each topic
//! - New subscribers immediately receive stored retained messages upon subscription
//! - Only one retained message per topic (new retained messages replace old ones)
//! - Retained messages persist even when publisher disconnects
//!
//! **Message Clearing:**
//! - `clear_retained()` sends empty payload with retain=true to remove stored message
//! - After clearing, new subscribers receive no retained messages
//! - The typed client automatically ignores empty payloads (clear events)
//!
//! **Multi-client Behavior:**
//! - Multiple clients can subscribe to same topic independently
//! - Each gets its own copy of retained messages upon subscription
//! - Non-retained messages only go to currently active subscribers
//!
//! ## Demo Timeline (20 seconds total):
//!
//! ```text
//! t=0s:  Publisher sends retained message #1 → stored by broker
//! t=1s:  Subscriber-1 connects → receives retained message #1
//! t=5s:  Publisher sends retained message #2 → replaces #1 in broker storage
//! t=6s:  Subscriber-2 connects → receives retained message #2 (not #1)
//! t=10s: Publisher sends non-retained message #3 → only Supervisor sees it
//! t=11s: Subscriber-3 connects → receives retained message #2 (not #3)
//! t=15s: Publisher clears retained message → broker storage now empty
//! t=16s: Subscriber-4 connects → receives nothing (no retained message available)
//! ```
//!
//! Topic pattern: "demo/retain" (fixed topic, no parameters)
//! Expected output: Shows how retained messages behave with different connection timing

mod shared;

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, MqttClient, MqttClientError, MqttConnection,
};
use mqtt_typed_client_macros::mqtt_topic;

/// Demo message payload with embedded information for simplicity
#[derive(Encode, Decode, Debug, Clone)]
struct DemoMessage {
	content: String, // All demo information embedded in human-readable text
}

/// Topic structure for retain demonstration
///
/// Pattern: "demo/retain" (fixed topic path, no parameters)
/// - Publisher publishes to: "demo/retain"
/// - Subscribers subscribe to: "demo/retain"
/// - All clients share the same topic for simplicity
#[mqtt_topic("demo/retain")]
pub struct RetainDemoTopic {
	payload: DemoMessage, // Automatically deserialized message payload
}

/// Helper function to create MQTT client connection with consistent error handling
async fn create_client(
	client_id: &str,
) -> Result<(MqttClient<BincodeSerializer>, MqttConnection), MqttClientError> {
	let connection_url = shared::config::build_url(client_id);
	MqttClient::<BincodeSerializer>::connect(&connection_url)
		.await
		.inspect_err(|e| {
			shared::config::print_connection_error(&connection_url, e)
		})
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	println!("=== MQTT Retain & Clear Demo ===\n");
	println!(
		"This demo shows how MQTT retained messages work with multiple \
		 clients."
	);
	println!(
		"Watch how subscribers connecting at different times receive \
		 different messages.\n"
	);

	// === CONCURRENT TASK LAUNCH ===
	// Launch all tasks concurrently to simulate real-world multi-client scenario
	// Each task represents a different MQTT client with different connection timing
	tokio::join!(
		async { run_publisher().await.unwrap() }, // Publishes retained and non-retained messages
		async { run_supervisor().await.unwrap() }, // Monitors all messages in real-time
		async { run_delayed_subscriber(1, "subscriber-1").await.unwrap() }, // Connects at t=1s (after first retained msg)
		async { run_delayed_subscriber(6, "subscriber-2").await.unwrap() }, // Connects at t=6s (after second retained msg)
		async { run_delayed_subscriber(11, "subscriber-3").await.unwrap() }, // Connects at t=11s (after non-retained msg)
		async { run_delayed_subscriber(18, "subscriber-4").await.unwrap() }, // Connects at t=18s (after clear_retained)
	);

	Ok(())
}

/// Main publisher logic demonstrating retained and non-retained messages
///
/// This function demonstrates the complete lifecycle of MQTT retained messages:
/// 1. Publishing retained messages (stored by broker)
/// 2. Publishing non-retained messages (immediate delivery only)
/// 3. Clearing retained messages (removing from broker storage)
async fn run_publisher() -> Result<(), Box<dyn std::error::Error>> {
	// === 1. CONNECTION ===
	println!("[PUBLISHER] Connecting to MQTT broker...");
	let (client, connection) = create_client("retain_publisher").await?;

	// Get typed topic client for RetainDemoTopic structure
	let topic_client = client.retain_demo_topic();

	// === 2. PUBLISHER SETUP ===
	// Create one publisher and use new convenience methods
	let publisher = topic_client.get_publisher()?;

	println!("[PUBLISHER] Publishers configured, starting demo sequence...\n");

	// === 3. INITIAL DELAY ===
	// Brief delay to ensure supervisor subscriber is ready before we start publishing.
	// This is a demo simplification - production code should use proper synchronization.
	tokio::time::sleep(Duration::from_secs(1)).await;

	// === 4. FIRST RETAINED MESSAGE ===
	let msg1 = DemoMessage {
		content: "Retained message #1: First stored message".to_string(),
	};

	println!("[PUBLISHER] t=0s: Publishing retained message #1");
	println!(
		"           → Broker will store this message for future subscribers"
	);
	publisher.publish_retain(&msg1).await?;

	// Wait 5 seconds for demonstration timing
	tokio::time::sleep(Duration::from_secs(5)).await;

	// === 5. SECOND RETAINED MESSAGE (REPLACEMENT) ===
	let msg2 = DemoMessage {
		content: "Retained message #2: Updated stored message".to_string(),
	};

	println!("[PUBLISHER] t=5s: Publishing retained message #2 (replaces #1)");
	println!(
		"           → Broker replaces previous retained message with this one"
	);
	publisher.publish_retain(&msg2).await?;

	tokio::time::sleep(Duration::from_secs(5)).await;

	// === 6. NON-RETAINED MESSAGE ===
	let msg3 = DemoMessage {
		content: "Non-retained message #3: Temporary message".to_string(),
	};

	println!("[PUBLISHER] t=10s: Publishing non-retained message #3");
	println!(
		"            → Only currently connected subscribers will receive this"
	);
	publisher.publish_normal(&msg3).await?;

	tokio::time::sleep(Duration::from_secs(5)).await;

	// === 7. CLEAR RETAINED MESSAGE ===
	println!("[PUBLISHER] t=15s: Clearing retained message from broker");
	println!(
		"            → Sends empty payload with retain=true to remove stored \
		 message"
	);
	publisher.clear_retained().await?;

	tokio::time::sleep(Duration::from_secs(5)).await;
	println!("[PUBLISHER] t=20s: Demo sequence completed\n");

	// === 8. CLEANUP ===
	connection.shutdown().await?;
	Ok(())
}

/// Supervisor monitors all messages in real-time to show complete message flow
///
/// The supervisor connects immediately and stays connected throughout the demo,
/// allowing it to see all messages (retained and non-retained) as they are published.
/// This provides a complete view of the message flow for comparison with delayed subscribers.
async fn run_supervisor() -> Result<(), Box<dyn std::error::Error>> {
	// === 1. CONNECTION ===
	let (client, connection) = create_client("retain_supervisor").await?;

	// Get typed topic client for RetainDemoTopic structure
	let topic_client = client.retain_demo_topic();

	// === 2. SUBSCRIPTION ===
	// Subscribe to topic: "demo/retain"
	// Since supervisor connects before any messages are published,
	// it will see all messages in real-time but no initial retained messages
	let mut subscriber = topic_client.subscribe().await?;
	println!(
		"[SUPERVISOR] Started continuous monitoring of topic: demo/retain"
	);
	println!("[SUPERVISOR] Will show all messages as they are published\n");

	// === 3. MESSAGE MONITORING LOOP ===
	// Monitor for demo duration (25 seconds total gives buffer for cleanup)
	let _result = tokio::time::timeout(Duration::from_secs(25), async {
		while let Some(result) = subscriber.receive().await {
			match result {
				| Ok(msg) => {
					let now =
						chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
					println!(
						"[SUPERVISOR] {}: Received: '{}'",
						now, msg.payload.content
					);
				}
				| Err(e) => {
					eprintln!("[SUPERVISOR] Deserialization error: {e}");
				}
			}
		}
	})
	.await;

	// === 4. CLEANUP ===
	connection.shutdown().await?;
	Ok(())
}

/// Delayed subscriber connects at specific time to test retained message behavior
///
/// Each delayed subscriber demonstrates different aspects of retained message behavior:
/// - subscriber-1 (t=1s): Connects after first retained message → receives msg #1
/// - subscriber-2 (t=6s): Connects after second retained message → receives msg #2 (not #1)
/// - subscriber-3 (t=11s): Connects after non-retained message → receives msg #2 (not #3)  
/// - subscriber-4 (t=16s): Connects after clear_retained() → receives nothing
async fn run_delayed_subscriber(
	delay_seconds: u64,
	client_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	// === 1. DELAYED CONNECTION ===
	// Wait for the specified delay to simulate subscriber connecting at different times
	tokio::time::sleep(Duration::from_secs(delay_seconds)).await;

	let (client, connection) =
		create_client(&format!("retain_{client_id}")).await?;

	// Get typed topic client for RetainDemoTopic structure
	let topic_client = client.retain_demo_topic();

	// === 2. SUBSCRIPTION ===
	// Subscribe to topic: "demo/retain"
	// If broker has a retained message for this topic, it will be delivered immediately
	let mut subscriber = topic_client.subscribe().await?;

	let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
	println!(
		"[{}] {}: Connected at t={}s, checking for retained messages...",
		client_id.to_uppercase(),
		now,
		delay_seconds
	);

	// === 3. RETAINED MESSAGE CHECK ===
	// Wait briefly to see if broker delivers any retained messages
	// Retained messages are delivered immediately upon subscription if they exist
	let timeout_result =
		tokio::time::timeout(Duration::from_secs(3), subscriber.receive())
			.await;

	match timeout_result {
		// Successfully received a message
		| Ok(Some(Ok(msg))) => {
			let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
			println!(
				"[{}] {}: ✓ Received retained message: '{}'",
				client_id.to_uppercase(),
				now,
				msg.payload.content
			);
		}
		// Received a message but deserialization failed
		| Ok(Some(Err(e))) => {
			println!(
				"[{}] ✗ Error receiving message: {}",
				client_id.to_uppercase(),
				e
			);
		}
		// No message received within timeout (broker has no retained message)
		| Ok(None) | Err(_) => {
			let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
			println!(
				"[{}] {}: ✗ No retained message received (broker storage \
				 empty)",
				client_id.to_uppercase(),
				now
			);
		}
	}

	// === 4. CLEANUP ===
	connection.shutdown().await?;
	Ok(())
}
