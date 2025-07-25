//! # Topic Definitions for Modular MQTT Example
//!
//! This module demonstrates how to organize MQTT topic definitions
//! in a clean, modular way that separates concerns and promotes reusability.
//!
//! ## Topic Structure
//!
//! Pattern: `sensors/{location}/{sensor_type}/{device_id}/data`
//!
//! Examples:
//! - `sensors/Home/floor/42/data` - Floor sensor in home location, device ID 42
//! - `sensors/Office/ceiling/100/data` - Ceiling sensor in office, device ID 100
//! - `sensors/Warehouse/door/370/data` - Door sensor in warehouse, device ID 370
//!
//! ## Generated API
//!
//! The `#[mqtt_topic]` macro generates:
//! - Type-safe publisher: `client.temperature_topic().get_publisher("Home", "floor", 42)`
//! - Wildcard subscriber: `client.temperature_topic().subscribe()` → `sensors/+/+/+/data`
//! - Filtered subscriber: `client.temperature_topic().subscription().for_device_id(42).subscribe()`
//! - Message parsing: Automatic deserialization to `TemperatureTopic` struct

use std::sync::Arc;

use bincode::{Decode, Encode};
use mqtt_typed_client::topic::topic_match::TopicMatch;
use mqtt_typed_client_macros::mqtt_topic;

/// Temperature sensor data payload
///
/// Contains the actual sensor measurements and metadata.
/// Uses bincode serialization for efficient binary encoding over MQTT.
#[derive(Debug, Clone, Decode, Encode)]
pub struct TemperatureReading {
	/// Unique identifier for the sensor device
	pub device_id: usize,
	/// Temperature measurement in Celsius
	pub temperature: f32,
	/// Optional humidity percentage (0-100)
	pub humidity: Option<f32>,
	/// Optional battery level percentage (0-100)
	pub battery_level: Option<u8>,
}

/// MQTT topic definition for temperature sensor data
///
/// Topic pattern: `sensors/{location}/{sensor_type}/{device_id}/data`
///
/// The `#[mqtt_topic]` macro generates a complete typed client API:
/// - Publishers with type-safe parameter validation
/// - Subscribers with automatic message parsing
/// - Flexible subscription patterns (wildcard vs. filtered)
///
/// # Generated Methods
///
/// ```rust,ignore
/// // Get topic client
/// let topic_client = mqtt_client.temperature_topic();
///
/// // Publishing
/// let publisher = topic_client.get_publisher("Home", "floor", 42)?;
/// publisher.publish(&temperature_data).await?;
///
/// // Subscribing to all sensors
/// let mut all_subscriber = topic_client.subscribe().await?;
///
/// // Subscribing to specific device
/// let mut device_subscriber = topic_client
///     .subscription()
///     .for_device_id(42)
///     .subscribe().await?;
/// ```
#[derive(Debug)]
#[mqtt_topic("sensors/{location}/{sensor_type}/{device_id}/data")]
pub struct TemperatureTopic {
	/// Physical location of the sensor (e.g., "Home", "Office", "Warehouse")
	pub location: String,
	/// Type of sensor (e.g., "floor", "ceiling", "door")
	pub sensor_type: String,
	/// Unique device identifier
	pub device_id: usize,
	/// Sensor measurement data
	pub payload: TemperatureReading,
	/// MQTT topic match information (automatically populated)
	pub topic: Arc<TopicMatch>,
}
