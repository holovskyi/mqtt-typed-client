//! # All Serializers Demo
//!
//! Demonstrates that all 9 available serializers can be used with the MQTT client.
//! This example tests connection and basic messaging with every implemented serializer.
//!
//! All serializers are tested since this example requires all features to be enabled.

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, CborSerializer, FlexbuffersSerializer, JsonSerializer,
	MessagePackSerializer, MessageSerializer, MqttClient, PostcardSerializer,
	ProtobufSerializer, RonSerializer,
};
use serde::{Deserialize, Serialize};

// Test message that works with all serializers
// Different serializers require different derive macros:
// - Serde-compatible: Serialize, Deserialize (JSON, MessagePack, CBOR, Postcard, RON, Flexbuffers)
// - Bincode: Encode, Decode
// - Protobuf: requires generated types from .proto files
// - Cap'n Proto: requires generated types from .capnp files
#[derive(Serialize, Deserialize, Encode, Decode, Debug, Clone)]
struct TestMessage {
	text: String,
	id: u32,
}

// Remove the separate BincodeTestMessage - we'll use TestMessage for everything

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("Testing all 8 available serializers...\n");

	// Test all serializers with full publish/subscribe cycle
	test_full_cycle::<BincodeSerializer>("Bincode").await?;
	test_full_cycle::<JsonSerializer>("JSON").await?;
	test_full_cycle::<MessagePackSerializer>("MessagePack").await?;
	test_full_cycle::<CborSerializer>("CBOR").await?;
	test_full_cycle::<PostcardSerializer>("Postcard").await?;
	test_full_cycle::<RonSerializer>("RON").await?;
	test_full_cycle::<FlexbuffersSerializer>("Flexbuffers").await?;

	// Test Protobuf (connection only - requires generated types for messaging)
	test_connection_only::<ProtobufSerializer>("Protobuf").await?;

	println!("\nAll 8 serializers tested successfully!");
	println!("   • 7 serializers with full publish/subscribe functionality");
	println!(
		"   • 1 serializer with connection-only test (requires generated \
		 types for messaging)"
	);
	Ok(())
}

// Test serializers with full publish/subscribe cycle (like 000_hello_world.rs)
async fn test_full_cycle<S>(
	name: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
	S: MessageSerializer<TestMessage> + Default + 'static,
	<S as MessageSerializer<TestMessage>>::SerializeError: std::fmt::Debug,
	<S as MessageSerializer<TestMessage>>::DeserializeError: std::fmt::Debug,
{
	println!("Testing {name} serializer...");

	let url = format!(
		"mqtt://localhost:1883?client_id=test_{}",
		name.to_lowercase()
	);
	let (client, connection) = MqttClient::<S>::connect(&url).await?;

	// Create publisher and subscriber using correct API
	let topic = format!("test/{}", name.to_lowercase());
	let publisher = client.get_publisher::<TestMessage>(&topic)?;
	let mut subscriber =
		client.subscribe::<TestMessage>(topic.as_str()).await?;

	// Small delay to ensure subscription is ready
	tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

	// Publish test message
	let message = TestMessage {
		text: format!("Hello from {name} serializer!"),
		id: 42,
	};

	publisher.publish(&message).await?;

	// Wait for message and verify deserialization
	println!("   Waiting for message...");
	if let Some((topic_match, result)) = subscriber.receive().await {
		match result {
			| Ok(received_message) => {
				println!(
					"   Received from {}: {} (id: {})",
					topic_match.topic_path(),
					received_message.text,
					received_message.id
				);
				println!("{name} (serialize + deserialize successful)");
			}
			| Err(e) => {
				println!("{name} (deserialization error: {e:?})");
				return Err(format!("Deserialization failed: {e:?}").into());
			}
		}
	} else {
		println!("{name} (no message received)");
		return Err("No message received".into());
	}

	connection.shutdown().await?;
	Ok(())
}

// Test serializers that only support connection (require generated types for messaging)
async fn test_connection_only<S>(
	name: &str,
) -> Result<(), Box<dyn std::error::Error>>
where S: Default + Clone + Send + Sync + 'static {
	println!("Testing {name} serializer (connection only)...");

	let url = format!(
		"mqtt://localhost:1883?client_id=test_{}",
		name.to_lowercase().replace(" ", "_")
	);
	let (client, connection) = MqttClient::<S>::connect(&url).await?;

	// Connection successful - but we can't create publishers without generated types
	// This proves the serializer can be instantiated and used for connections
	let _ = client; // Acknowledge we have a working client

	connection.shutdown().await?;

	println!(
		"{name} (connection successful, messaging requires generated types)"
	);
	Ok(())
}
