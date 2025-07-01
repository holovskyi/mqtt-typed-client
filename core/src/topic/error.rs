//! Error types and utilities for the topic module
//!
//! This module contains the composite error type and shared constants
//! for the entire topic module, while individual error types remain
//! in their respective modules.

use thiserror::Error;

use super::topic_matcher::TopicMatcherError;
use super::topic_pattern_item::TopicPatternError;
use super::topic_pattern_path::TopicFormatError;
use super::topic_router::TopicRouterError;

/// Comprehensive error type for all topic-related operations
///
/// This enum aggregates all possible errors that can occur within the topic module,
/// providing a single error type for the public API while maintaining detailed
/// error information from each submodule.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicError {
	/// Topic pattern parsing or validation error
	#[error("Topic pattern error: {0}")]
	Pattern(#[from] TopicPatternError),

	/// Topic formatting error when substituting parameters
	#[error("Topic format error: {0}")]
	Format(#[from] TopicFormatError),

	/// Topic matching operation error
	#[error("Topic matcher error: {0}")]
	Matcher(#[from] TopicMatcherError),

	/// Topic routing operation error
	#[error("Topic router error: {0}")]
	Router(#[from] TopicRouterError),
}

/// Convenient Result type for topic operations
pub type TopicResult<T> = Result<T, TopicError>;

/// Convenient Result type for pattern operations
pub type PatternResult<T> = Result<T, TopicPatternError>;

/// Convenient Result type for matcher operations
pub type MatcherResult<T> = Result<T, TopicMatcherError>;

/// Convenient Result type for router operations
pub type RouterResult<T> = Result<T, TopicRouterError>;

/// Topic processing limits and constants
pub mod limits {
	/// Maximum topic nesting depth allowed
	pub const MAX_TOPIC_DEPTH: usize = 32;

	/// Maximum length of a single topic segment
	pub const MAX_SEGMENT_LENGTH: usize = 256;

	/// Maximum total topic path length
	pub const MAX_TOPIC_LENGTH: usize = 1024;
}

/// Validation utilities for topic operations
pub mod validation {
	use super::limits::*;
	use super::{TopicMatcherError, TopicPatternError};

	/// Validates topic path for basic constraints
	pub fn validate_topic_path(path: &str) -> Result<(), TopicMatcherError> {
		if path.is_empty() {
			return Err(TopicMatcherError::EmptyTopicPath);
		}

		if path.len() > MAX_TOPIC_LENGTH {
			return Err(TopicMatcherError::invalid_utf8(format!(
				"Topic path too long: {} > {}",
				path.len(),
				MAX_TOPIC_LENGTH
			)));
		}

		if !path.is_ascii() {
			return Err(TopicMatcherError::invalid_utf8(
				"Non-ASCII characters in topic path".to_string(),
			));
		}

		let segments: Vec<&str> = path.split('/').collect();
		if segments.len() > MAX_TOPIC_DEPTH {
			return Err(TopicMatcherError::invalid_segment(
				format!("depth-{}", segments.len()),
				0,
			));
		}

		for (index, segment) in segments.iter().enumerate() {
			if segment.len() > MAX_SEGMENT_LENGTH {
				return Err(TopicMatcherError::invalid_segment(
					segment.to_string(),
					index,
				));
			}

			if segment.contains('\0') {
				return Err(TopicMatcherError::invalid_segment(
					format!("null-byte-in-{segment}"),
					index,
				));
			}
		}

		Ok(())
	}

	/// Validates topic pattern for subscription constraints
	pub fn validate_pattern_for_subscription(
		pattern: &str,
	) -> Result<(), TopicPatternError> {
		// Basic validation first
		if pattern.is_empty() || pattern.trim().is_empty() {
			return Err(TopicPatternError::EmptyTopic);
		}

		let segments: Vec<&str> = pattern.split('/').collect();
		if segments.len() > MAX_TOPIC_DEPTH {
			return Err(TopicPatternError::wildcard_usage(format!(
				"Pattern too deep: {} segments > {}",
				segments.len(),
				MAX_TOPIC_DEPTH
			)));
		}

		Ok(())
	}
}
