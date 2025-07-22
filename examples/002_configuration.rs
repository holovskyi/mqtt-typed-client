//! # Configuration Example - MQTT Typed Client
//!
//! Demonstrates how to configure MQTT client parameters:
//! - Connection settings (broker, credentials, keep-alive, clean session)
//! - Client behavior (event loop capacity, timeouts, cache sizes, channel capacities)
//! - Publisher settings (QoS levels, message retention)
//! - Subscriber settings (QoS levels, topic caching)
//!
//! This example shows a single sensor publishing temperature data
//! and a monitor receiving it, with detailed configuration explanations.

mod shared;

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientConfig};
use mqtt_typed_client_macros::mqtt_topic;
use rumqttc::QoS;

/// Sensor data payload containing temperature measurement
#[derive(Encode, Decode, Debug)]
struct TemperatureReading {
	temperature: f32,
}

/// Topic structure for temperature sensor data
///
/// Pattern: "sensors/temperature/{location}/{sensor_id}"
/// Example: "sensors/temperature/kitchen/sensor_001"
/// Payload: TemperatureReading with temperature value
#[derive(Debug)]
#[mqtt_topic("sensors/temperature/{location}/{sensor_id}")]
pub struct TemperatureTopic {
	location: String,
	sensor_id: String,
	payload: TemperatureReading,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	println!("Starting MQTT Configuration Example...\n");

	// === 1. CONNECTION CONFIGURATION ===
	// Get host and port from environment configuration
	let (host, port) = shared::config::get_mqtt_broker_host_port();
	let client_id = shared::config::get_client_id("temp_sensor");
	
	println!("Configuring MQTT client for {host}:{port}");
	
	let mut config = MqttClientConfig::<BincodeSerializer>::new(
		&client_id,
		&host,
		port,
	);

	// Configure connection parameters
	config
		.connection
		.set_keep_alive(Duration::from_secs(30)) // Send ping every 30 seconds
		.set_clean_session(false); // Resume session on reconnect
	// .set_credentials("username", "password") // Uncomment for authenticated brokers

	// === 2. CLIENT SETTINGS CONFIGURATION ===
	config.settings.event_loop_capacity = 50; // Channel capacity for event processing
	config.settings.connection_timeout_millis = 8000; // Connection timeout (8 seconds)
	config.settings.topic_cache_size = 50; // Maximum cached topic patterns

	// Connect with configuration
	let (client, connection) = MqttClient::connect_with_config(config)
		.await
		.inspect_err(|e| {
			shared::config::print_connection_error(&format!("{host}:{port}"), e);
		})?;
	println!("✓ Connected with custom configuration\n");

	// === 3. SUBSCRIBER CONFIGURATION ===
	let topic_client = client.temperature_topic();
	let mut subscriber = topic_client
		.subscription()
		// QoS 1: Guaranteed delivery (at least once)
		.with_qos(QoS::AtLeastOnce)
		// LRU cache for topic matching: improves performance
		// when the same topic patterns repeat frequently
		.with_cache(10)
		.subscribe()
		.await?;

	println!("✓ Subscribed to temperature sensors with cache\n");

	// === 4. PUBLISHER CONFIGURATION ===
	let publisher = topic_client
		.get_publisher("kitchen", "sensor_001")?
		.with_qos(QoS::AtLeastOnce) // QoS 1: Guaranteed delivery (at least once)
		.with_retain(true); // Store last message for new subscribers

	// Small delay to ensure subscription is ready
	tokio::time::sleep(Duration::from_millis(100)).await;

	// === 5. PUBLISH TEMPERATURE DATA ===
	let reading = TemperatureReading { temperature: 22.5 };
	publisher.publish(&reading).await?;
	println!("✓ Published temperature reading with retention\n");

	// === 6. RECEIVE FROM SUBSCRIPTION ===
	// Wait for message
	if let Some(Ok(temp_data)) = subscriber.receive().await {
		println!("=== Received Configuration Demo ===");
		println!("  Location: {}", temp_data.location);
		println!("  Sensor ID: {}", temp_data.sensor_id);
		println!("  Temperature: {}°C", temp_data.payload.temperature);
	}

	// Graceful shutdown
	connection.shutdown().await?;
	println!("\n✓ Goodbye!");
	Ok(())
}
