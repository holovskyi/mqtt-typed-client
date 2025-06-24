//! MQTT client module
//!
//! This module provides high-level MQTT client functionality including
//! typed publishers, subscribers, and async client management.

pub mod async_client;
pub mod error;
pub mod publisher;
pub mod subscriber;
pub mod config;

// Re-export commonly used types for convenience
pub use async_client::MqttClient;
pub use error::MqttClientError;
pub use config::MqttClientConfig;
pub use publisher::MqttPublisher;
pub use subscriber::MqttSubscriber;

// Connection type is re-exported from the root level
pub use crate::connection::MqttConnection;
