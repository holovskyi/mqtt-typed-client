//! MQTT client module
//!
//! This module provides high-level MQTT client functionality including
//! typed publishers, subscribers, and async client management.

pub mod async_client;
pub mod error;
pub mod publisher;
pub mod subscriber;

// Re-export commonly used types for convenience
pub use async_client::MqttClient;
pub use error::MqttClientError;
pub use publisher::TopicPublisher;
pub use subscriber::TypedSubscriber;
