//! Topic handling module - re-exported from mqtt-topic-engine
//!
//! This module provides components for working with MQTT topic patterns,
//! including parsing, matching, and routing messages based on topic patterns.

// Re-export everything from mqtt-topic-engine
pub use mqtt_topic_engine::{
	// Main types
	CacheStrategy,
	MatcherResult,
	PatternResult,
	RouterResult,

	SubscriptionId,

	// Error types
	TopicError,
	TopicFormatError,
	// Matching types
	TopicMatch,
	TopicMatcherError,
	TopicPath,

	TopicPatternError,
	TopicPatternItem,
	TopicPatternPath,
	// Result type aliases
	TopicResult,
	TopicRouter,
	TopicRouterError,

	// Error utilities
	limits,
	validation,
};

// Create module aliases for backward compatibility with internal imports
// like `use crate::topic::topic_match::TopicMatch;`
/// Topic matching types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_match {
	pub use mqtt_topic_engine::{TopicMatch, TopicPath};
}

/// Topic pattern path types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_pattern_path {
	pub use mqtt_topic_engine::{TopicFormatError, TopicPatternPath};
}
