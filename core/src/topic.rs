//! Topic handling module - re-exported from mqtt-topic-engine
//!
//! This module provides components for working with MQTT topic patterns,
//! including parsing, matching, and routing messages based on topic patterns.

// Re-export everything from mqtt-topic-engine
// `TopicMatchError` is not re-exported from the engine root, only from its
// `topic_match` submodule, so bring it into the flat `topic::` namespace
// explicitly for backward compatibility with v0.1.0.
pub use mqtt_topic_engine::topic_match::TopicMatchError;
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

// Create module aliases for backward compatibility with v0.1.0 submodule paths
// (e.g. `use mqtt_typed_client_core::topic::topic_router::TopicRouter;`) and with
// internal imports like `use crate::topic::topic_match::TopicMatch;`.
//
// Note: the internal matcher machinery (`TopicMatcherNode`, `Len`) is intentionally
// NOT re-exported — it was public only incidentally and is not part of the stable API.

/// Topic matching types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_match {
	pub use mqtt_topic_engine::topic_match::{
		TopicMatch, TopicMatchError, TopicPath,
	};
}

/// Topic pattern path types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_pattern_path {
	pub use mqtt_topic_engine::{TopicFormatError, TopicPatternPath};
}

/// Topic error types and validation utilities
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod error {
	pub use mqtt_topic_engine::error::{
		MatcherResult, PatternResult, RouterResult, TopicError, TopicResult,
		limits, validation,
	};
}

/// Topic pattern item types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_pattern_item {
	pub use mqtt_topic_engine::topic_pattern_item::{
		TopicPatternError, TopicPatternItem,
	};
}

/// Topic matcher error type
///
/// Re-exported from mqtt-topic-engine for backward compatibility. The matcher
/// node/trait internals are intentionally not re-exported.
pub mod topic_matcher {
	pub use mqtt_topic_engine::topic_matcher::TopicMatcherError;
}

/// Topic router types
///
/// Re-exported from mqtt-topic-engine for backward compatibility.
pub mod topic_router {
	pub use mqtt_topic_engine::topic_router::{
		SubscriptionId, TopicRouter, TopicRouterError,
	};
}
