//! # MQTT Typed Client
//!
//! A Rust library providing a typed MQTT client with pattern-based routing
//! and automatic subscription management.
//!
//! ## Features
//!
//! - **Typed Publishers and Subscribers**: Type-safe message publishing and subscription
//! - **Pattern-based Routing**: Support for MQTT wildcard patterns (`+`, `#`)
//! - **Automatic Subscription Management**: Handles subscription lifecycle automatically
//! - **Graceful Shutdown**: Proper resource cleanup and connection termination
//! - **Async/Await Support**: Built on top of `tokio` for async operations
//! - **Error Handling**: Comprehensive error types with retry logic
//! - **Message Serialization**: Pluggable serialization (Bincode included)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use mqtt_typed_client::{MqttAsyncClient, BincodeSerializer};
//! use serde::{Deserialize, Serialize};
//! use bincode::{Encode, Decode};
//!
//! #[derive(Serialize, Deserialize, Encode, Decode, Debug)]
//! struct SensorData {
//!     temperature: f64,
//!     humidity: f64,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create MQTT client
//!     let client = MqttAsyncClient::<BincodeSerializer>::new(
//!         "mqtt://broker.hivemq.com:1883"
//!     ).await?;
//!
//!     // Create a typed publisher
//!     let publisher = client.get_publisher::<SensorData>("sensors/temperature")?;
//!
//!     // Create a typed subscriber
//!     let mut subscriber = client.subscribe::<SensorData>("sensors/+").await?;
//!
//!     // Publish data
//!     let data = SensorData { temperature: 23.5, humidity: 45.0 };
//!     publisher.publish(&data).await?;
//!
//!     // Receive data
//!     if let Some((topic, result)) = subscriber.receive().await {
//!         match result {
//!             Ok(sensor_data) => println!("Received from {}: {:?}", topic, sensor_data),
//!             Err(e) => eprintln!("Deserialization error: {:?}", e),
//!         }
//!     }
//!
//!     // Graceful shutdown
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Pattern Matching
//!
//! The library supports MQTT topic pattern matching:
//!
//! - `+` matches a single topic level (e.g., `sensors/+/temperature`)
//! - `#` matches multiple topic levels (e.g., `sensors/#`)
//!
//! ## Custom Serialization
//!
//! Implement the `MessageSerializer` trait for custom serialization:
//!
//! ```rust
//! use mqtt_typed_client::MessageSerializer;
//!
//! #[derive(Clone, Default)]
//! struct JsonSerializer;
//!
//! impl<T> MessageSerializer<T> for JsonSerializer
//! where
//!     T: serde::Serialize + serde::de::DeserializeOwned + 'static,
//! {
//!     type SerializeError = serde_json::Error;
//!     type DeserializeError = serde_json::Error;
//!
//!     fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
//!         serde_json::to_vec(data)
//!     }
//!
//!     fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
//!         serde_json::from_slice(bytes)
//!     }
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

// Core modules
pub mod client;
pub mod message_serializer;
pub mod routing;
pub mod topic;
// Re-export commonly used types for convenience
pub use macros::mqtt_topic_subscriber;

#[cfg(test)]
mod test_macro;

pub use client::{
	MqttClient, MqttClientError, TopicPublisher, TypedSubscriber,
};
pub use message_serializer::{BincodeSerializer, MessageSerializer};
pub use routing::{
	CacheStrategy, SendError, Subscriber, SubscriptionConfig,
	SubscriptionError, SubscriptionManagerHandler,
};
// Re-export external types that users commonly need
pub use rumqttc::QoS;
pub use topic::{
	SubscriptionId, TopicError, TopicPatternError, TopicPatternPath,
	TopicRouter, TopicRouterError, limits, validation,
};

/// Result type alias for operations that may fail with MqttClientError
pub type Result<T> = std::result::Result<T, MqttClientError>;

/// Prelude module for convenient imports
pub mod prelude {
	//! Convenience re-exports for common functionality
	//!
	//! This module provides a convenient way to import the most commonly used
	//! types and traits from the mqtt_typed_client library.
	//!
	//! ```rust
	//! use mqtt_typed_client::prelude::*;
	//! ```

	pub use crate::{
		BincodeSerializer, MessageSerializer, MqttClient, MqttClientError, QoS,
		Result, SubscriptionId, TopicPatternPath, TopicPublisher,
		TypedSubscriber,
	};
}
