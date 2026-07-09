//! Message routing and subscription management module
//!
//! This module provides functionality for managing MQTT subscriptions,
//! routing messages to subscribers, and handling subscription lifecycle.

/// Routing and subscription error types
pub mod error;
/// Low-level subscriber implementation
pub mod subscriber;
pub(crate) mod subscription_manager;

// Re-export commonly used types for convenience
pub use error::SubscriptionError;
pub use subscriber::ReceiveEvent;
pub(crate) use subscriber::Subscriber;
pub use subscription_manager::SubscriptionConfig;
pub(crate) use subscription_manager::{
	SubscriptionManagerActor, SubscriptionManagerController,
	SubscriptionManagerHandler,
};

// Re-export for internal crate usage only
pub use crate::topic::CacheStrategy;
