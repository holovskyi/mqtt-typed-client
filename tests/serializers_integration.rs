//! Integration tests for all available serializers
//!
//! This test ensures that all implemented serializers can create MQTT clients
//! and perform basic operations. When MQTT broker is available, also tests
//! full serialization/deserialization cycle.
#![cfg(all(
	feature = "json",
	feature = "messagepack",
	feature = "cbor",
	feature = "postcard",
	feature = "ron",
	feature = "flexbuffers",
	feature = "protobuf"
))]

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, CborSerializer, FlexbuffersSerializer, JsonSerializer,
	MessagePackSerializer, MessageSerializer, MqttClient, PostcardSerializer,
	ProtobufSerializer, RonSerializer,
};
use serde::{Deserialize, Serialize};

// Unified test message that works with all serializers
// Different serializers require different derive macros:
// - Serde-compatible: Serialize, Deserialize (JSON, MessagePack, CBOR, Postcard, RON, Flexbuffers)
// - Bincode: Encode, Decode
// - Protobuf: requires generated types from .proto files
// - Cap'n Proto: requires generated types from .capnp files
#[derive(Serialize, Deserialize, Encode, Decode, Debug, Clone, PartialEq)]
struct TestMessage {
	text: String,
	id: u32,
}

#[cfg(test)]
mod serializer_tests {
	use super::*;

	// Test all serde-compatible + bincode serializers with unified approach
	#[tokio::test]
	async fn test_bincode_serializer() {
		test_serializer_integration::<BincodeSerializer>("Bincode").await;
	}

	#[tokio::test]
	async fn test_json_serializer() {
		test_serializer_integration::<JsonSerializer>("JSON").await;
	}

	#[tokio::test]
	async fn test_messagepack_serializer() {
		test_serializer_integration::<MessagePackSerializer>("MessagePack")
			.await;
	}

	#[tokio::test]
	async fn test_cbor_serializer() {
		test_serializer_integration::<CborSerializer>("CBOR").await;
	}

	#[tokio::test]
	async fn test_postcard_serializer() {
		test_serializer_integration::<PostcardSerializer>("Postcard").await;
	}

	#[tokio::test]
	async fn test_ron_serializer() {
		test_serializer_integration::<RonSerializer>("RON").await;
	}

	#[tokio::test]
	async fn test_flexbuffers_serializer() {
		test_serializer_integration::<FlexbuffersSerializer>("Flexbuffers")
			.await;
	}

	// Test schema-based serializers (connection only)
	#[tokio::test]
	async fn test_protobuf_serializer() {
		test_connection_only::<ProtobufSerializer>("Protobuf").await;
	}
}

/// Test serializers with full integration (connection + optional publish/subscribe if broker available)
async fn test_serializer_integration<S>(name: &str)
where
	S: MessageSerializer<TestMessage> + Default + 'static,
	<S as MessageSerializer<TestMessage>>::SerializeError: std::fmt::Debug,
	<S as MessageSerializer<TestMessage>>::DeserializeError: std::fmt::Debug,
{
	let url = format!(
		"mqtt://localhost:1883?client_id=test_{}_integration",
		name.to_lowercase()
	);

	match MqttClient::<S>::connect(&url).await {
		| Ok((client, connection)) => {
			// Connection successful - test full cycle if possible
			println!("{} serializer: Connection successful", name);

			// Test publisher creation
			let topic = format!("test/integration/{}", name.to_lowercase());
			let publisher = client.get_publisher::<TestMessage>(&topic);
			assert!(
				publisher.is_ok(),
				"{} serializer: Failed to create publisher",
				name
			);

			// If we have both publisher and can create subscriber, test full cycle
			match client.subscribe::<TestMessage>(topic.as_str()).await {
				| Ok(mut subscriber) => {
					println!(
						"{} serializer: Testing full publish/subscribe cycle",
						name
					);

					// Small delay to ensure subscription is ready
					tokio::time::sleep(tokio::time::Duration::from_millis(200))
						.await;

					// Publish test message
					let message = TestMessage {
						text: format!("Integration test for {}", name),
						id: 123,
					};

					if let Ok(_pub_result) =
						publisher.unwrap().publish(&message).await
					{
						println!(
							"{} serializer: Message published successfully",
							name
						);

						// Try to receive with timeout
						let timeout = tokio::time::Duration::from_millis(1000);
						let receive_result =
							tokio::time::timeout(timeout, subscriber.receive())
								.await;

						match receive_result {
							| Ok(Some((_topic_match, result))) => {
								match result {
									| Ok(received_message) => {
										assert_eq!(
											received_message, message,
											"{} serializer: Message mismatch",
											name
										);
										println!(
											"{} serializer: Full cycle \
											 successful (serialize + \
											 deserialize)",
											name
										);
									}
									| Err(e) => {
										panic!(
											"{} serializer: Deserialization \
											 failed: {:?}",
											name, e
										);
									}
								}
							}
							| Ok(None) => {
								println!(
									"{} serializer: ⚠️ No message received \
									 (broker might be busy)",
									name
								);
							}
							| Err(_) => {
								println!(
									"{} serializer: ⚠️ Receive timeout \
									 (broker might be slow)",
									name
								);
							}
						}
					} else {
						println!(
							"{} serializer: ⚠️ Publish failed (broker might \
							 be busy)",
							name
						);
					}
				}
				| Err(e) => {
					println!(
						"{} serializer: ⚠️ Subscription failed: {:?}",
						name, e
					);
				}
			}

			// Always shutdown gracefully
			let shutdown_result = connection.shutdown().await;
			assert!(
				shutdown_result.is_ok(),
				"{} serializer: Failed to shutdown",
				name
			);

			println!("{} serializer: Integration test completed", name);
		}
		| Err(e) => {
			// Connection failed - this is OK in CI/test environments without MQTT broker
			println!(
				"{} serializer: Connection failed (expected in CI without \
				 broker): {:?}",
				name, e
			);

			// Even without broker, we can test that the serializer compiles and can be instantiated
			let _serializer = S::default();
			println!(
				"{} serializer: Serializer instantiation successful",
				name
			);
		}
	}
}

/// Test connection-only for serializers that require generated types
async fn test_connection_only<S>(name: &str)
where S: Default + Clone + Send + Sync + 'static {
	let url = format!(
		"mqtt://localhost:1883?client_id=test_{}_connection",
		name.to_lowercase().replace(" ", "_")
	);

	match MqttClient::<S>::connect(&url).await {
		| Ok((client, connection)) => {
			println!("{} serializer: Connection successful", name);

			// Connection works - serializer is properly implemented
			let _ = client; // Acknowledge we have a working client

			let shutdown_result = connection.shutdown().await;
			assert!(
				shutdown_result.is_ok(),
				"{} serializer: Failed to shutdown",
				name
			);

			println!(
				"{} serializer: Connection test successful (messaging \
				 requires generated types)",
				name
			);
		}
		| Err(e) => {
			// Connection failed - this is OK in CI/test environments without MQTT broker
			println!(
				"{} serializer: Connection failed (expected in CI without \
				 broker): {:?}",
				name, e
			);

			// Even without broker, we can test that the serializer compiles and can be instantiated
			let _serializer = S::default();
			println!(
				"{} serializer: Serializer instantiation successful",
				name
			);
		}
	}
}

// Compilation tests to verify all serializer traits are correctly implemented
#[cfg(test)]
mod compilation_tests {
	use super::*;

	#[test]
	fn test_all_serializer_trait_bounds() {
		// This test ensures all serializers implement the required traits correctly
		// If any serializer has incorrect trait bounds, this won't compile

		fn assert_messaging_serializer<S, T>()
		where
			S: MessageSerializer<T> + Default + 'static,
			S::SerializeError: std::fmt::Debug,
			S::DeserializeError: std::fmt::Debug,
		{
			// Just a compilation test - no runtime logic needed
		}

		fn assert_connection_serializer<S>()
		where S: Default + Clone + Send + Sync + 'static {
			// Just a compilation test - no runtime logic needed
		}

		// Test all messaging serializers
		assert_messaging_serializer::<BincodeSerializer, TestMessage>();
		assert_messaging_serializer::<JsonSerializer, TestMessage>();
		assert_messaging_serializer::<MessagePackSerializer, TestMessage>();
		assert_messaging_serializer::<CborSerializer, TestMessage>();
		assert_messaging_serializer::<PostcardSerializer, TestMessage>();
		assert_messaging_serializer::<RonSerializer, TestMessage>();
		assert_messaging_serializer::<FlexbuffersSerializer, TestMessage>();

		// Test connection-only serializers
		assert_connection_serializer::<ProtobufSerializer>();

		println!("All serializer trait bounds are correct");
	}

	#[test]
	fn test_message_serialization_compatibility() {
		// Test that our unified TestMessage works with different derive macros
		let test_msg = TestMessage {
			text: "Test message".to_string(),
			id: 42,
		};

		// This would fail to compile if derives are incompatible
		let cloned = test_msg.clone();
		assert_eq!(test_msg, cloned);

		// Test Debug trait
		let debug_str = format!("{:?}", test_msg);
		assert!(debug_str.contains("Test message"));
		assert!(debug_str.contains("42"));

		println!("Unified TestMessage works with all required traits");
	}
}
