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
//! use mqtt_typed_client::{MqttClient, BincodeSerializer};
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
//!     let (client, connection) = MqttClient::<BincodeSerializer>::connect(
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
pub mod connection;
pub mod message_serializer;
pub mod routing;
/// Structured MQTT subscribers with automatic topic parameter extraction
pub mod structured;
pub mod topic;

// === Core Public API ===
// Main client types
pub use client::{MqttClient, MqttClientConfig, MqttClientError};
pub use connection::MqttConnection;

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

// Low-level subscriber types (for advanced usage)
pub use client::{MqttPublisher, MqttSubscriber};

// Topic pattern types (for manual pattern handling)
pub use topic::{TopicPatternPath, TopicPatternError};

/// Result type alias for operations that may fail with MqttClientError
pub type Result<T> = std::result::Result<T, MqttClientError>;

/// Prelude module for convenient imports
///
/// This module provides the most commonly used types for typical MQTT applications.
/// Use this when you want to import everything you need with a single line:
///
/// ```rust
/// use mqtt_typed_client::prelude::*;
/// ```
pub mod prelude {
	//! Essential types for most MQTT applications

	pub use crate::{
		BincodeSerializer, MqttClient, MqttClientConfig, MqttClientError,
		MqttConnection, MessageSerializer, QoS, Result,
	};
}

/// Advanced types and utilities for complex use cases
///
/// This module contains types that are useful for advanced scenarios:
/// - Custom topic pattern handling
/// - Low-level subscription management 
/// - Internal error types
/// - Validation utilities
///
/// ```rust
/// use mqtt_typed_client::advanced::*;
/// ```
pub mod advanced {
	//! Advanced types for complex use cases

	pub use crate::{
		TopicPatternPath, TopicPatternError, MqttPublisher, MqttSubscriber,
		CacheStrategy, SubscriptionConfig,
	};
	
	// Topic utilities
	pub use crate::topic::{
		limits, validation, TopicError, TopicRouterError, SubscriptionId,
	};
	
	// Routing internals for power users
	pub use crate::routing::{
		SendError, SubscriptionError, Subscriber,
	};
}

/// Error types used throughout the library
///
/// Re-exports all error types in one convenient location for error handling.
///
/// ```rust
/// use mqtt_typed_client::errors::*;
/// ```
pub mod errors {
	//! All error types used in the library

	pub use crate::{
		MqttClientError, MessageConversionError, TopicPatternError,
	};
	
	// Topic-related errors
	pub use crate::topic::{TopicError, TopicRouterError};
	
	// Routing errors
	pub use crate::routing::{SendError, SubscriptionError};
}
