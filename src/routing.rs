//! Message routing and subscription management module
//!
//! This module provides functionality for managing MQTT subscriptions,
//! routing messages to subscribers, and handling subscription lifecycle.

pub mod error;
pub mod subscriber;
pub mod subscription_manager;

// Re-export commonly used types for convenience
pub use error::{SendError, SubscriptionError};
pub use subscriber::Subscriber;
pub use subscription_manager::{
	SubscriptionManagerActor, SubscriptionManagerController,
	SubscriptionManagerHandler, CacheStrategy, SubscriptionConfig
};
