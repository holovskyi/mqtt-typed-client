//! Integration test for the per-topic serializer macro attribute.
//!
//! Verifies the `#[mqtt_topic("...", serializer = JsonSerializer)]` path
//! end-to-end: a topic declared with an explicit serializer (different from the
//! client's default) round-trips publish -> subscribe -> deserialize correctly.
//!
//! Unlike `examples/102_multi_serializer_macro.rs` (which only prints), this
//! test asserts payload integrity and runs under `cargo test`. It degrades
//! gracefully when no MQTT broker is available (e.g. CI without a broker).
//!
//! Requires a running broker on `localhost:1883` to exercise the full cycle;
//! otherwise it only verifies that the macro-generated code compiles and the
//! client can be constructed.

use mqtt_typed_client::{
	BincodeSerializer, JsonSerializer, MqttClient, ReceiveEvent,
};
use mqtt_typed_client_macros::mqtt_topic;
use serde::{Deserialize, Serialize};

/// JSON payload (needs serde derives for `JsonSerializer`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct DeviceStatus {
	message: String,
	count: i32,
}

/// Topic that explicitly overrides the client's default serializer (Bincode)
/// with JSON via the macro attribute.
#[mqtt_topic("test/macro/devices/{device_id}/status", serializer = JsonSerializer)]
struct DeviceStatusTopic {
	device_id: String,
	payload: DeviceStatus,
}

#[tokio::test]
async fn test_macro_per_topic_serializer_round_trip() {
	let base = std::env::var("MQTT_BROKER_URL")
		.unwrap_or_else(|_| "mqtt://localhost:1883".to_string());
	let url = format!("{base}?client_id=test_macro_serializer");

	let (client, connection) =
		match MqttClient::<BincodeSerializer>::connect(&url).await {
			| Ok(pair) => pair,
			| Err(e) => {
				// When MQTT_REQUIRE_BROKER is set (CI with a live broker), a
				// failed connection is a real failure, not a graceful skip.
				assert!(
					std::env::var("MQTT_REQUIRE_BROKER").is_err(),
					"MQTT_REQUIRE_BROKER is set but connecting to the broker \
					 failed: {e:?}"
				);
				// No broker — OK locally. The macro path already compiled,
				// which is itself meaningful coverage.
				println!(
					"Connection failed (expected without a broker): {e:?}"
				);
				return;
			}
		};

	// Topic with JSON serializer override (via macro attribute).
	let mut status_sub = DeviceStatusTopic::subscribe(&client)
		.await
		.expect("subscribe to JSON-serialized topic");

	// Give the subscription time to settle on the broker.
	tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

	let sent = DeviceStatus {
		message: "operational".to_string(),
		count: 7,
	};
	DeviceStatusTopic::publish(&client, "device-xyz", &sent)
		.await
		.expect("publish JSON-serialized message");

	// We are connected here, so the round-trip MUST complete — a closed stream
	// or a timeout is a real failure, not a "broker busy" skip. (Graceful skip
	// only applies when there is no broker at all, handled above.)
	let timeout = tokio::time::Duration::from_millis(2000);
	match tokio::time::timeout(timeout, status_sub.receive()).await {
		| Ok(Some(ReceiveEvent::Message(msg))) => {
			assert_eq!(
				msg.device_id, "device-xyz",
				"topic parameter must round-trip"
			);
			assert_eq!(
				msg.payload, sent,
				"JSON payload must round-trip via macro serializer"
			);
			println!("✓ JSON macro serializer round-trip verified");
		}
		| Ok(Some(ReceiveEvent::DecodeFailed(err))) => {
			panic!("failed to deserialize JSON-serialized message: {err:?}")
		}
		| Ok(Some(_)) => panic!("unexpected non-message stream event"),
		| Ok(None) => {
			panic!("subscriber stream closed before receiving message")
		}
		| Err(_) => panic!("timed out waiting for round-trip message"),
	}

	connection.shutdown().await.expect("shutdown");
}
