//! MQTT client module
//!
//! This module provides high-level MQTT client functionality including
//! typed publishers, subscribers, and async client management.

/// Asynchronous MQTT client implementation
pub mod async_client;
pub mod config;
/// Protocol-neutral connection options
pub mod connection_options;
/// Client error types
pub mod error;
pub mod last_will;
/// Typed MQTT publishers
pub mod publisher;
/// Typed MQTT subscribers
pub mod subscriber;
/// Subscription builder for flexible configuration
pub mod subscription_builder;

// Re-export commonly used types for convenience
pub use async_client::MqttClient;
pub use config::{ClientSettings, MqttClientConfig};
#[cfg(feature = "unstable-backend-api")]
pub use connection_options::backend;
pub use connection_options::{
	ConnectionOptions, Credentials, ProtocolVersion, RustlsClientConfig,
	SessionPolicy, TlsConfig, Transport,
};
pub use error::{
	BackendError, ClientOperationError, ConnectReasonCode,
	ConnectionEstablishmentError, MqttClientError, UrlParseError,
};
pub use last_will::TypedLastWill;
pub use publisher::MqttPublisher;
pub use subscriber::{DecodeFailure, IncomingMessage, MqttSubscriber};
pub use subscription_builder::SubscriptionBuilder;

// Connection type is available from the root level
// Use: mqtt_typed_client::MqttConnection
