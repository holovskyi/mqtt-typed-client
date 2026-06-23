//! # MQTT Topic Engine
//!
#![doc = mqtt_typed_client_doc_macros::include_md_transformed!("README.md", transform = "readme")]
//!
#![warn(missing_docs)]

// Public modules
pub mod cache_strategy;
pub mod error;
pub mod qos;
pub mod topic_match;
pub mod topic_pattern_item;
pub mod topic_pattern_path;

// Router modules (optional)
#[cfg(feature = "router")]
pub mod topic_matcher;
#[cfg(feature = "router")]
pub mod topic_router;

// Tests modules - вони залишаються приватними
#[cfg(all(test, feature = "router"))]
mod topic_matcher_tests;
#[cfg(test)]
mod topic_pattern_item_tests;
#[cfg(test)]
mod topic_pattern_path_tests;

// Re-export main types for convenience
pub use cache_strategy::CacheStrategy;
// Router-specific re-exports
#[cfg(feature = "router")]
pub use error::{MatcherResult, RouterResult};
pub use error::{PatternResult, TopicError, TopicResult, limits, validation};
pub use qos::QoS;
pub use topic_match::{TopicMatch, TopicPath};
#[cfg(feature = "router")]
pub use topic_matcher::TopicMatcherError;
pub use topic_pattern_item::{TopicPatternError, TopicPatternItem};
pub use topic_pattern_path::{TopicFormatError, TopicPatternPath};
#[cfg(feature = "router")]
pub use topic_router::{SubscriptionId, TopicRouter, TopicRouterError};
