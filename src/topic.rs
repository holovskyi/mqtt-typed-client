//! Topic handling module
//!
//! This module provides components for working with MQTT topic patterns,
//! including parsing, matching, and routing messages based on topic patterns.

// Submodules
pub mod error;
pub mod topic_matcher;
pub mod topic_pattern_path;
pub mod topic_router;
pub mod topic_match;

#[cfg(test)]
mod topic_matcher_tests;

// Re-export commonly used types for convenience
pub use error::{
	MatcherResult, PatternResult, RouterResult, TopicError, TopicResult,
};
// Re-export constants and validation utilities
pub use error::{limits, validation};
pub use topic_matcher::{IsEmpty, Len, TopicMatcherError, TopicMatcherNode};
pub use topic_pattern_path::{
	TopicPatternError, TopicPatternItem, TopicPatternPath,
};
pub use topic_router::{SubscriptionId, TopicRouter, TopicRouterError};
