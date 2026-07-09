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
//! use mqtt_typed_client_core::{MqttClient, MqttClientConfig, BincodeSerializer, ReceiveEvent};
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
//!     use mqtt_typed_client_core::SessionPolicy;
//!     let mut config = MqttClientConfig::new("my_client", "broker.hivemq.com", 1883);
//!     config.connection.keep_alive = std::time::Duration::from_secs(30);
//!     config.connection.session = SessionPolicy::CleanPerConnection;
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
//!     if let Some(event) = subscriber.receive().await {
//!         match event {
//!             ReceiveEvent::Message((topic, sensor_data)) => {
//!                 println!("Received from {}: {:?}", topic, sensor_data)
//!             }
//!             ReceiveEvent::DecodeFailed((topic, e)) => {
//!                 eprintln!("Deserialization error at {}: {:?}", topic, e)
//!             }
//!             ReceiveEvent::Lagged { missed } => {
//!                 eprintln!("Lagged: {missed} messages dropped")
//!             }
//!             _ => {}
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
//! use mqtt_typed_client_core::MessageSerializer;
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
// SEMVER-EXEMPT raw backend access (escape hatch)
#[cfg(feature = "unstable-backend-api")]
pub use client::backend;
pub use client::{
	ClientSettings, ConnectionOptions, Credentials, MqttClient,
	MqttClientConfig, MqttClientError, ProtocolVersion, RustlsClientConfig,
	SessionPolicy, TlsConfig, Transport, TypedLastWill,
};
// High-level typed publishers and subscribers
pub use client::{MqttPublisher, MqttSubscriber, SubscriptionBuilder};
pub use connection::MqttConnection;
// Message serialization
#[cfg(feature = "bincode-serializer")]
pub use message_serializer::BincodeSerializer;
#[cfg(feature = "cbor")]
pub use message_serializer::CborSerializer;
#[cfg(feature = "flexbuffers")]
pub use message_serializer::FlexbuffersSerializer;
#[cfg(feature = "json")]
pub use message_serializer::JsonSerializer;
#[cfg(feature = "messagepack")]
pub use message_serializer::MessagePackSerializer;
pub use message_serializer::MessageSerializer;
#[cfg(feature = "postcard")]
pub use message_serializer::PostcardSerializer;
#[cfg(feature = "protobuf")]
pub use message_serializer::ProtobufSerializer;
#[cfg(feature = "ron")]
pub use message_serializer::RonSerializer;
// Protocol-neutral QoS (from mqtt-topic-engine); conversion to the backend's
// QoS happens internally at the rumqttc boundary.
pub use mqtt_topic_engine::QoS;
// === Advanced API ===
// Advanced subscription configuration
pub use routing::{ReceiveEvent, SubscriptionConfig};
// The backend's rustls stack, so a `rustls::ClientConfig` for
// `Transport::Tls(..)` can be built without a version-matched rustls
// dependency of your own. The rustls major version tracks the backend's TLS
// stack (documented semver-coupled exception).
#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
pub use rumqttc::tokio_rustls::rustls;
// Structured subscribers (macro support)
pub use structured::{
	FromMqttMessage, MessageConversionError, MqttTopicSubscriber,
	extract_topic_parameter,
};
pub use topic::CacheStrategy;
// Topic pattern types (for manual pattern handling)
pub use topic::{TopicError, TopicPatternError, TopicPatternPath};

/// Result type alias for operations that may fail with MqttClientError
pub type Result<T> = std::result::Result<T, MqttClientError>;

/// Prelude module for convenient imports
///
/// Essential types for most MQTT applications.
/// This module provides the most commonly used types for typical MQTT applications.
/// Use this when you want to import everything you need with a single line:
///
/// ```rust
/// use mqtt_typed_client_core::prelude::*;
/// ```
pub mod prelude {

	#[cfg(feature = "bincode-serializer")]
	pub use crate::BincodeSerializer;
	#[cfg(feature = "cbor")]
	pub use crate::CborSerializer;
	#[cfg(feature = "flexbuffers")]
	pub use crate::FlexbuffersSerializer;
	#[cfg(feature = "json")]
	pub use crate::JsonSerializer;
	#[cfg(feature = "messagepack")]
	pub use crate::MessagePackSerializer;
	#[cfg(feature = "postcard")]
	pub use crate::PostcardSerializer;
	#[cfg(feature = "protobuf")]
	pub use crate::ProtobufSerializer;
	#[cfg(feature = "ron")]
	pub use crate::RonSerializer;
	pub use crate::{
		ClientSettings, ConnectionOptions, MessageSerializer, MqttClient,
		MqttClientConfig, MqttClientError, MqttConnection, QoS, Result,
		SessionPolicy, SubscriptionBuilder, Transport, TypedLastWill,
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
/// use mqtt_typed_client_core::advanced::*;
/// ```
pub mod advanced {

	// High-level routing errors only
	pub use crate::routing::SubscriptionError;
	// Topic utilities
	pub use crate::topic::{
		SubscriptionId, TopicRouterError, limits, validation,
	};
	pub use crate::{
		CacheStrategy, MqttPublisher, MqttSubscriber, ReceiveEvent,
		SubscriptionConfig, TopicError, TopicPatternPath,
	};
}

/// Error types used throughout the library
///
/// All error types used in the library.
/// Re-exports all error types in one convenient location for error handling.
///
/// ```rust
/// use mqtt_typed_client_core::errors::*;
/// ```
pub mod errors {

	// Client and connection errors
	pub use crate::client::{
		BackendError, ClientOperationError, ConnectReasonCode,
		ConnectionEstablishmentError, UrlParseError,
	};
	// High-level routing errors
	pub use crate::routing::SubscriptionError;
	// Topic-related errors - specific types for advanced usage
	pub use crate::topic::{TopicMatcherError, TopicRouterError};
	pub use crate::{
		MessageConversionError, MqttClientError, TopicError, TopicPatternError,
	};
}
