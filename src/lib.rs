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
//! use mqtt_typed_client::{MqttClient, MqttClientConfig, BincodeSerializer};
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
//!     // Simple connection using URL
//!     let (client, connection) = MqttClient::<BincodeSerializer>::connect(
//!         "mqtt://broker.hivemq.com:1883?client_id=my_client"
//!     ).await?;
//!
//!     // Advanced configuration
//!     let mut config = MqttClientConfig::new("my_client", "broker.hivemq.com", 1883);
//!     config.connection.set_keep_alive(std::time::Duration::from_secs(30));
//!     config.connection.set_clean_session(true);
//!     config.settings.topic_cache_size = 500;
//!     
//!     let (client, connection) = MqttClient::<BincodeSerializer>::connect_with_config(config).await?;
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
//!     connection.shutdown().await?;
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
//! ### Publisher Limitations
//!
//! **Note**: Multi-level wildcards (`#`) can only be used for subscriptions,
//! not for publishing. This is because publishers need to generate specific
//! topic strings, while `#` represents a variable number of topic segments.
//!
//! For patterns containing `#`, use the `mqtt_topic` macro with explicit
//! `subscriber` mode, or create separate structs for publishing and subscribing.
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

// Core modules
pub mod client;
pub mod connection;
pub mod message_serializer;
pub mod routing;
/// Structured MQTT subscribers with automatic topic parameter extraction
pub mod structured;
pub mod topic;

// === Core Public API ===
// Main client types
pub use client::{MqttClient, MqttClientConfig, ClientSettings, MqttClientError};
pub use connection::MqttConnection;

// Re-export rumqttc types for advanced configuration
pub use rumqttc::MqttOptions;

// Message serialization
pub use message_serializer::{BincodeSerializer, MessageSerializer};

// Structured subscribers (macro support)
pub use structured::{
	FromMqttMessage, MessageConversionError, MqttTopicSubscriber,
	extract_topic_parameter,
};

// Essential external types
pub use rumqttc::QoS;

// === Advanced API ===
// Advanced subscription configuration
pub use routing::{CacheStrategy, SubscriptionConfig};

// High-level typed publishers and subscribers
pub use client::{MqttPublisher, MqttSubscriber};

// Topic pattern types (for manual pattern handling)
pub use topic::{TopicPatternPath, TopicError};

/// Result type alias for operations that may fail with MqttClientError
pub type Result<T> = std::result::Result<T, MqttClientError>;

/// Prelude module for convenient imports
///
/// Essential types for most MQTT applications.
/// This module provides the most commonly used types for typical MQTT applications.
/// Use this when you want to import everything you need with a single line:
///
/// ```rust
/// use mqtt_typed_client::prelude::*;
/// ```
pub mod prelude {

	pub use crate::{
		BincodeSerializer, MqttClient, MqttClientConfig, ClientSettings, MqttClientError,
		MqttConnection, MessageSerializer, MqttOptions, QoS, Result,
	};
}

/// Advanced types and utilities for complex use cases
///
/// Advanced types for complex use cases.
/// This module contains types that are useful for advanced scenarios:
/// - Custom topic pattern handling
/// - Advanced error types
/// - Validation utilities
///
/// ```rust
/// use mqtt_typed_client::advanced::*;
/// ```
pub mod advanced {

	pub use crate::{
		TopicPatternPath, TopicError, MqttPublisher, MqttSubscriber,
		CacheStrategy, SubscriptionConfig,
	};
	
	// Topic utilities
	pub use crate::topic::{
		limits, validation, TopicRouterError, SubscriptionId,
	};
	
	// High-level routing errors only
	pub use crate::routing::{
		SubscriptionError,
	};
}

/// Error types used throughout the library
///
/// All error types used in the library.
/// Re-exports all error types in one convenient location for error handling.
///
/// ```rust
/// use mqtt_typed_client::errors::*;
/// ```
pub mod errors {

	pub use crate::{
		MqttClientError, MessageConversionError, TopicError,
	};
	
	// Topic-related errors - specific types for advanced usage
	pub use crate::topic::{
		TopicPatternError, TopicMatcherError, TopicRouterError
	};
	
	// High-level routing errors
	pub use crate::routing::{SubscriptionError};
}
