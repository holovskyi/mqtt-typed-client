//! MQTT client module
//!
//! This module provides high-level MQTT client functionality including
//! typed publishers, subscribers, and async client management.

/// Asynchronous MQTT client implementation
pub mod async_client;
/// Client error types
pub mod error;
/// Typed MQTT publishers
pub mod publisher;
/// Typed MQTT subscribers
pub mod subscriber;
/// Subscription builder for flexible configuration
pub mod subscription_builder;
pub mod config;
pub mod last_will;

// Re-export commonly used types for convenience
pub use async_client::MqttClient;
pub use error::MqttClientError;
pub use config::{MqttClientConfig, ClientSettings};
pub use publisher::MqttPublisher;
pub use subscriber::MqttSubscriber;
pub use subscription_builder::SubscriptionBuilder;
pub use last_will::TypedLastWill;

// Connection type is available from the root level
// Use: mqtt_typed_client::MqttConnection
