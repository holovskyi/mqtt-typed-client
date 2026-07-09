//! # Message Metadata - MQTT Typed Client
//!
//! Demonstrates the optional `meta` and `topic` fields on a `#[mqtt_topic]`
//! struct:
//! - `meta: MessageMeta` — per-message protocol metadata (QoS, retain, dup).
//! - `topic: TopicMatch` — the concrete matched topic (wildcards resolved).
//!
//! Both are **Arc-adaptive**: declare the bare type (`MessageMeta` /
//! `TopicMatch`) to get an owned value, or `Arc<...>` for zero-copy. This
//! example uses the owned forms for readability; for hot paths with many
//! subscribers on the same topic, prefer `Arc<MessageMeta>` / `Arc<TopicMatch>`
//! to avoid the clone.
//!
//! Topic pattern: "telemetry/{device}"

mod shared;

use bincode::{Decode, Encode};
use mqtt_typed_client::topic::topic_match::TopicMatch;
use mqtt_typed_client::{
	BincodeSerializer, MessageMeta, MqttClient, ReceiveEvent,
};
use mqtt_typed_client_macros::mqtt_topic;

#[derive(Encode, Decode, Debug)]
struct Reading {
	celsius: f64,
}

/// A topic struct that also captures the matched topic and message metadata.
///
/// `device` comes from the wildcard; `payload` is deserialized; `topic` and
/// `meta` are filled by the library. Owned forms (`TopicMatch` / `MessageMeta`)
/// are used here — swap to `Arc<...>` for zero-copy.
#[mqtt_topic("telemetry/{device}")]
pub struct TelemetryTopic {
	device: String,
	payload: Reading,
	topic: TopicMatch,
	meta: MessageMeta,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	shared::tracing::setup(None);

	let connection_url = shared::config::build_url("message_metadata");
	println!("Connecting to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	let topic_client = client.telemetry_topic();
	let mut subscriber = topic_client.subscribe().await?;
	println!("Subscribed to: telemetry/+");

	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	topic_client
		.publish("sensor_42", &Reading { celsius: 21.7 })
		.await?;

	println!("Waiting for telemetry...");
	if let Some(ReceiveEvent::Message(reading)) = subscriber.receive().await {
		println!("Device {}:", reading.device);
		println!("   Concrete topic: {}", reading.topic.topic_path());
		println!("   Temperature: {} C", reading.payload.celsius);
		println!(
			"   Metadata: qos={:?}, retain={}, dup={}",
			reading.meta.qos, reading.meta.retain, reading.meta.dup
		);
	}

	connection.shutdown().await?;
	println!("\nGoodbye!");
	Ok(())
}
