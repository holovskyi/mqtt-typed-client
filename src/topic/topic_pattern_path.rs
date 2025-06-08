use std::convert::TryFrom;
use std::slice::Iter;
use std::usize;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopicPatternItem {
	Str(String),
	Plus,
	Hash,
}

/// Error types for topic pattern parsing
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicPatternError {
	/// Hash wildcard (#) used not at the end of the pattern
	#[error(
		"Invalid topic pattern '{pattern}': # wildcard can only be the last \
		 segment"
	)]
	HashPosition { pattern: String },

	/// Wildcard characters (+ or #) used incorrectly
	#[error("Invalid wildcard usage: {usage}")]
	WildcardUsage { usage: String },

	/// Empty topic is not valid
	#[error("Topic pattern cannot be empty")]
	EmptyTopic,
}

impl TopicPatternError {
	/// Creates a new HashPosition error
	pub fn hash_position(pattern: impl Into<String>) -> Self {
		Self::HashPosition {
			pattern: pattern.into(),
		}
	}

	/// Creates a new WildcardUsage error
	pub fn wildcard_usage(usage: impl Into<String>) -> Self {
		Self::WildcardUsage {
			usage: usage.into(),
		}
	}

	/// Returns true if this error is a validation error (client-side)
	pub fn is_validation_error(&self) -> bool {
		true // All pattern errors are validation errors
	}

	/// Returns the error type for categorization
	pub fn error_type(&self) -> &'static str {
		match self {
			| TopicPatternError::HashPosition { .. } => "hash_position",
			| TopicPatternError::WildcardUsage { .. } => "wildcard_usage",
			| TopicPatternError::EmptyTopic => "empty_topic",
		}
	}
}

impl TopicPatternItem {
	pub fn as_str(&self) -> &str {
		match self {
			| TopicPatternItem::Str(s) => s,
			| TopicPatternItem::Plus => "+",
			| TopicPatternItem::Hash => "#",
		}
	}
}

impl From<&TopicPatternItem> for String {
	fn from(item: &TopicPatternItem) -> Self {
		item.as_str().to_string()
	}
}

impl std::fmt::Display for TopicPatternItem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<&str> for TopicPatternItem {
	type Error = TopicPatternError;
	fn try_from(item: &str) -> Result<Self, Self::Error> {
		let res = match item {
			| "+" => TopicPatternItem::Plus,
			| "#" => TopicPatternItem::Hash,
			| _ if item.contains(['+', '#']) => {
				return Err(TopicPatternError::wildcard_usage(item));
			}
			| _ => TopicPatternItem::Str(item.to_string()),
		};
		Ok(res)
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopicPatternPath {
	segments: Vec<TopicPatternItem>,
}

impl TopicPatternPath {
	pub fn is_empty(&self) -> bool {
		self.segments.is_empty()
	}

	pub fn iter(&self) -> Iter<TopicPatternItem> {
		self.segments.iter()
	}

	pub fn len(&self) -> usize {
		self.segments.len()
	}

	pub fn str_len(segments: &[TopicPatternItem]) -> usize {
		if segments.is_empty() {
			return 0;
		}
		(segments.len() - 1) + // slashes count
		segments.iter().map(|s| s.as_str().len()).sum::<usize>()
	}

	pub fn slice(&self) -> &[TopicPatternItem] {
		&self.segments
	}

	pub fn push(
		&mut self,
		segment: TopicPatternItem,
	) -> Result<&Self, TopicPatternError> {
		if let Some(TopicPatternItem::Hash) = self.segments.last() {
			let error = format!("Can't add item ({segment}) after # in {self}");
			return Err(TopicPatternError::hash_position(error));
		}
		self.segments.push(segment);
		Ok(self)
	}

	pub fn segments_to_string(segments: &[TopicPatternItem]) -> String {
		// Convert segments to strings and join them with "/"
		if segments.is_empty() {
			return String::new();
		}
		let mut result = String::with_capacity(Self::str_len(segments));
		segments.iter().enumerate().for_each(|(i, segment)| {
			if i > 0 {
				result.push('/');
			}
			result.push_str(segment.as_str());
		});
		result
	}
}

impl TryFrom<&str> for TopicPatternPath {
	type Error = TopicPatternError;

	fn try_from(topic: &str) -> Result<Self, Self::Error> {
		if topic.is_empty() || topic.trim().is_empty() {
			return Err(TopicPatternError::EmptyTopic);
		}

		let segments: Result<Vec<_>, _> =
			topic.split('/').map(TopicPatternItem::try_from).collect();

		let segments = segments?;

		if let Some(hash_pos) =
			segments.iter().position(|s| *s == TopicPatternItem::Hash)
		{
			if hash_pos != segments.len() - 1 {
				return Err(TopicPatternError::hash_position(topic));
			}
		}
		Ok(Self { segments })
	}
}

impl TryFrom<&[TopicPatternItem]> for TopicPatternPath {
	type Error = TopicPatternError;

	fn try_from(segments: &[TopicPatternItem]) -> Result<Self, Self::Error> {
		if let Some(hash_pos) =
			segments.iter().position(|s| *s == TopicPatternItem::Hash)
		{
			if hash_pos != segments.len() - 1 {
				let path = Self::segments_to_string(segments);
				return Err(TopicPatternError::hash_position(path));
			}
		}
		Ok(Self {
			segments: segments.to_vec(),
		})
	}
}

impl std::fmt::Display for TopicPatternPath {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// Convert segments to strings and join them with "/"
		let path = Self::segments_to_string(&self.segments);
		write!(f, "{path}")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn str_to_topic_pattern_path(
		topic: &str,
	) -> Result<TopicPatternPath, TopicPatternError> {
		topic.try_into()
	}

	#[test]
	fn test_simple_string_pattern() {
		let result = str_to_topic_pattern_path("simple/path").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("simple".to_string()),
				TopicPatternItem::Str("path".to_string())
			]
		);
	}

	#[test]
	fn test_pattern_with_star() {
		let result = str_to_topic_pattern_path("devices/+/status").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("devices".to_string()),
				TopicPatternItem::Plus,
				TopicPatternItem::Str("status".to_string())
			]
		);
	}

	#[test]
	fn test_pattern_with_hash() {
		let result = str_to_topic_pattern_path("sensors/#").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("sensors".to_string()),
				TopicPatternItem::Hash
			]
		);
	}

	#[test]
	fn test_pattern_with_both_wildcards() {
		let result = str_to_topic_pattern_path("home/+/device/#").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("home".to_string()),
				TopicPatternItem::Plus,
				TopicPatternItem::Str("device".to_string()),
				TopicPatternItem::Hash
			]
		);
	}

	#[test]
	fn test_empty_string() {
		let result = str_to_topic_pattern_path("");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), TopicPatternError::EmptyTopic);
	}

	#[test]
	fn test_only_wildcards() {
		let result_star = str_to_topic_pattern_path("+").unwrap();
		assert_eq!(result_star.segments, vec![TopicPatternItem::Plus]);

		let result_hash = str_to_topic_pattern_path("#").unwrap();
		assert_eq!(result_hash.segments, vec![TopicPatternItem::Hash]);
	}

	#[test]
	fn test_consecutive_separators() {
		let result = str_to_topic_pattern_path("topic//subtopic");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments,
			vec![
				TopicPatternItem::Str("topic".to_string()),
				TopicPatternItem::Str("".to_string()),
				TopicPatternItem::Str("subtopic".to_string())
			]
		);
	}

	#[test]
	fn test_starting_with_separator() {
		let result = str_to_topic_pattern_path("/start");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments,
			vec![
				TopicPatternItem::Str("".to_string()),
				TopicPatternItem::Str("start".to_string())
			]
		);
	}

	#[test]
	fn test_ending_with_separator() {
		let result = str_to_topic_pattern_path("end/");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments,
			vec![
				TopicPatternItem::Str("end".to_string()),
				TopicPatternItem::Str("".to_string())
			]
		);
	}

	#[test]
	fn test_invalid_hash_wildcard_position() {
		let result = str_to_topic_pattern_path("invalid/#/pattern");
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			TopicPatternError::HashPosition {
				pattern: "invalid/#/pattern".to_string()
			}
		);
	}

	#[test]
	fn test_invalid_wildcards_format() {
		let result_double_star = str_to_topic_pattern_path("topic/++/subtopic");
		assert!(result_double_star.is_err());
		assert!(matches!(
			result_double_star.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));

		let result_double_hash = str_to_topic_pattern_path("topic/##");
		assert!(result_double_hash.is_err());
		assert!(matches!(
			result_double_hash.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));
	}

	#[test]
	fn test_wildcards_with_other_characters() {
		let result_star = str_to_topic_pattern_path("topic/a+b/subtopic");
		assert!(result_star.is_err());
		assert!(matches!(
			result_star.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));

		let result_hash = str_to_topic_pattern_path("topic/a#b");
		assert!(result_hash.is_err());
		assert!(matches!(
			result_hash.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));
	}

	#[test]
	fn test_very_long_pattern() {
		let long_pattern = "segment1/segment2/segment3/segment4/segment5/\
		                    segment6/segment7/segment8/segment9/segment10";
		let result = str_to_topic_pattern_path(long_pattern).unwrap();
		assert_eq!(result.segments.len(), 10);
	}

	#[test]
	fn test_unicode_characters() {
		let result = str_to_topic_pattern_path("пристрої/+/статус").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("пристрої".to_string()),
				TopicPatternItem::Plus,
				TopicPatternItem::Str("статус".to_string())
			]
		);
	}

	#[test]
	fn test_special_characters() {
		let result =
			str_to_topic_pattern_path("device-123/status@home").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str("device-123".to_string()),
				TopicPatternItem::Str("status@home".to_string())
			]
		);
	}

	#[test]
	fn test_display_implementation() {
		// Test simple string pattern
		let path = str_to_topic_pattern_path("simple/path").unwrap();
		assert_eq!(path.to_string(), "simple/path");

		// Test with wildcards
		let path = str_to_topic_pattern_path("devices/+/status").unwrap();
		assert_eq!(path.to_string(), "devices/+/status");

		// Test with hash wildcard
		let path = str_to_topic_pattern_path("sensors/#").unwrap();
		assert_eq!(path.to_string(), "sensors/#");

		// Test empty path
		let err = str_to_topic_pattern_path("");
		assert_eq!(err, Err(TopicPatternError::EmptyTopic));

		// Test / path
		let path = str_to_topic_pattern_path("/").unwrap();
		assert_eq!(path.to_string(), "/");

		// Test finish / path
		let path = str_to_topic_pattern_path("device/").unwrap();
		assert_eq!(path.to_string(), "device/");

		// Test with consecutive separators
		let path = str_to_topic_pattern_path("topic//subtopic").unwrap();
		assert_eq!(path.to_string(), "topic//subtopic");
	}
}
