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
/// Publisher builder for flexible configuration
pub mod publisher_builder;
/// Typed MQTT subscribers
pub mod subscriber;
/// Subscription builder for flexible configuration
pub mod subscription_builder;
pub mod config;

// Re-export commonly used types for convenience
pub use async_client::MqttClient;
pub use error::MqttClientError;
pub use config::{MqttClientConfig, ClientSettings};
pub use publisher::MqttPublisher;
pub use publisher_builder::PublisherBuilder;
pub use subscriber::MqttSubscriber;
pub use subscription_builder::SubscriptionBuilder;

// Connection type is available from the root level
// Use: mqtt_typed_client::MqttConnection
