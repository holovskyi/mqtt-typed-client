use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::num::NonZeroUsize;
use std::slice::Iter;
use std::sync::Arc;
use std::sync::Mutex;

use arcstr::{ArcStr, Substr};
use lru::LruCache;
use smallvec::SmallVec;
use thiserror::Error;

use crate::routing::subscription_manager::CacheStrategy;
use crate::topic::topic_match::{TopicMatch, TopicMatchError, TopicPath};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopicPatternItem {
	Str(Substr),
	Plus(Option<Substr>),
	Hash(Option<Substr>),
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
			| TopicPatternItem::Plus(_) => "+",
			| TopicPatternItem::Hash(_) => "#",
		}
	}

	pub fn as_wildcard(&self) -> Cow<str> {
		match self {
			| TopicPatternItem::Plus(None) => Cow::Borrowed("+"),
			| TopicPatternItem::Hash(None) => Cow::Borrowed("#"),
			| TopicPatternItem::Plus(Some(name)) => {
				Cow::Owned(format!("{{{name}}}"))
			}
			| TopicPatternItem::Hash(Some(name)) => {
				Cow::Owned(format!("{{{name}:#}}"))
			}
			| TopicPatternItem::Str(s) => Cow::Borrowed(s),
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

impl TryFrom<Substr> for TopicPatternItem {
	type Error = TopicPatternError;
	fn try_from(item: Substr) -> Result<Self, Self::Error> {
		let res = match item.as_str() {
			| "+" => TopicPatternItem::Plus(None),
			| "#" => TopicPatternItem::Hash(None),
			| _ if item.starts_with("{") && item.ends_with(":#}") => {
				let inner =
					item.trim_start_matches('{').trim_end_matches(":#}");
				if inner.is_empty() {
					return Err(TopicPatternError::wildcard_usage(
						item.as_str(),
					));
				}
				TopicPatternItem::Hash(Some(item.substr_from(inner)))
			}
			| _ if item.starts_with("{") && item.ends_with("}") => {
				let inner = item.trim_start_matches('{').trim_end_matches("}");
				if inner.is_empty() {
					return Err(TopicPatternError::wildcard_usage(
						item.as_str(),
					));
				}
				TopicPatternItem::Plus(Some(item.substr_from(inner)))
			}
			| _ if item.contains(['+', '#']) => {
				return Err(TopicPatternError::wildcard_usage(item.as_str()));
			}
			| _ => TopicPatternItem::Str(item),
		};
		Ok(res)
	}
}

#[derive(Debug)]
pub struct TopicPatternPath {
	topic_pattern: ArcStr, // original topic pattern as a string
	mqtt_pattern: ArcStr,  // mqtt topic pattern with wildcards "sensors/+/data"
	segments: Vec<TopicPatternItem>,
	match_cache: Option<Mutex<LruCache<ArcStr, Arc<TopicMatch>>>>,
}

impl TopicPatternPath {
	pub fn new_from_string(
		topic_pattern: impl Into<ArcStr>,
		cache_strategy: CacheStrategy,
	) -> Result<Self, TopicPatternError> {
		let topic_pattern = topic_pattern.into();
		if topic_pattern.is_empty() || topic_pattern.trim().is_empty() {
			return Err(TopicPatternError::EmptyTopic);
		}

		let segments: Result<Vec<_>, _> = topic_pattern
			.split('/')
			.map(|s| topic_pattern.substr_from(s))
			.map(TopicPatternItem::try_from)
			.collect();

		let segments = segments?;

		if let Some(hash_pos) = segments
			.iter()
			.position(|s| matches!(*s, TopicPatternItem::Hash(_)))
		{
			if hash_pos != segments.len() - 1 {
				return Err(TopicPatternError::hash_position(
					topic_pattern.as_str(),
				));
			}
		}
		let match_cache = match cache_strategy {
			| CacheStrategy::Lru(cache_size) => {
				Some(Mutex::new(LruCache::new(cache_size)))
			}
			| CacheStrategy::NoCache => None,
		};
		Ok(Self {
			topic_pattern,
			mqtt_pattern: ArcStr::from(Self::to_mqtt_subscription_pattern(
				&segments,
			)),
			segments,
			match_cache,
		})
	}

	#[cfg(test)]
	pub fn new_from_segments(
		segments: &[TopicPatternItem],
	) -> Result<Self, TopicPatternError> {
		let topic_pattern = ArcStr::from(Self::to_template_pattern(segments));
		let pattern = Self {
			mqtt_pattern: ArcStr::from(Self::to_mqtt_subscription_pattern(
				segments,
			)),
			topic_pattern: topic_pattern.clone(),
			segments: segments.to_vec(),
			match_cache: None,
		};
		if let Some(hash_pos) = segments
			.iter()
			.position(|s| matches!(*s, TopicPatternItem::Hash(_)))
		{
			if hash_pos != segments.len() - 1 {
				return Err(TopicPatternError::hash_position(
					topic_pattern.as_str(),
				));
			}
		}
		Ok(pattern)
	}

	pub fn mqtt_pattern(&self) -> ArcStr {
		self.mqtt_pattern.clone()
	}

	pub fn topic_pattern(&self) -> ArcStr {
		self.topic_pattern.clone()
	}

	pub fn is_empty(&self) -> bool {
		self.segments.is_empty()
	}

	pub fn iter(&self) -> Iter<TopicPatternItem> {
		self.segments.iter()
	}

	pub fn len(&self) -> usize {
		self.segments.len()
	}

	fn str_len(segments: &[TopicPatternItem]) -> usize {
		if segments.is_empty() {
			return 0;
		}
		(segments.len() - 1) + // slashes count
		segments.iter().map(|s| s.as_str().len()).sum::<usize>()
	}

	pub fn slice(&self) -> &[TopicPatternItem] {
		&self.segments
	}

	fn to_mqtt_subscription_pattern(segments: &[TopicPatternItem]) -> String {
		// Convert to MQTT wildcards: sensors/+/data
		if segments.is_empty() {
			return String::new();
		}
		let mut mqtt_topic = String::with_capacity(Self::str_len(segments));
		segments.iter().enumerate().for_each(|(i, segment)| {
			if i > 0 {
				mqtt_topic.push('/');
			}
			mqtt_topic.push_str(segment.as_str());
		});
		mqtt_topic
	}

	fn to_template_pattern(segments: &[TopicPatternItem]) -> String {
		// Convert to named wildcards: sensors/{sensor_id}/data
		if segments.is_empty() {
			return String::new();
		}
		let mut mqtt_topic = String::new();
		segments.iter().enumerate().for_each(|(i, segment)| {
			if i > 0 {
				mqtt_topic.push('/');
			}
			mqtt_topic.push_str(segment.as_wildcard().as_ref());
		});
		mqtt_topic
	}

	// pub fn wildcard_count(&self) -> usize {
	// 	self.segments
	// 		.iter()
	// 		.filter(|s| {
	// 			matches!(
	// 				s,
	// 				TopicPatternItem::Plus(_) | TopicPatternItem::Hash(_)
	// 			)
	// 		})
	// 		.count()
	// }

	// pub fn named_params_positions(&self) -> HashMap<String, usize> {
	// 	let mut named_params = HashMap::new();
	// 	let mut wildcad_index = 0;
	// 	for segment in &self.segments {
	// 		match segment {
	// 			| TopicPatternItem::Hash(opt_name)
	// 			| TopicPatternItem::Plus(opt_name) => {
	// 				opt_name.iter().for_each(|name| {
	// 					named_params.insert(name.clone(), wildcad_index);
	// 				});
	// 				wildcad_index += 1;
	// 			}
	// 			| TopicPatternItem::Str(_) => {
	// 				// Do nothing for regular string segments
	// 			}
	// 		}
	// 	}
	// 	named_params
	// }

	pub fn try_match(
		&self,
		topic: Arc<TopicPath>,
	) -> Result<Arc<TopicMatch>, TopicMatchError> {
		match &self.match_cache {
			| Some(cache_mutex) => {
				{
					let mut match_cache = cache_mutex.lock().unwrap();
					if let Some(cached_match) = match_cache.get(&topic.path) {
						return Ok(cached_match.clone());
					}
				}

				let topic_match = self.try_match_internal(topic.clone())?;
				let topic_match_arc = Arc::new(topic_match);
				{
					let mut match_cache = cache_mutex.lock().unwrap();
					match_cache
						.put(topic.path.clone(), Arc::clone(&topic_match_arc));
				}
				Ok(topic_match_arc)
			}
			| None => {
				let topic_match = self.try_match_internal(topic)?;
				Ok(Arc::new(topic_match))
			}
		}
	}

	#[allow(clippy::missing_docs_in_private_items)]
	fn try_match_internal(
		&self,
		topic: Arc<TopicPath>,
	) -> Result<TopicMatch, TopicMatchError> {
		let mut topic_index = 0;
		let mut params = SmallVec::new();
		let mut named_params = SmallVec::new();
		for (i, pattern_segment) in self.iter().enumerate() {
			match pattern_segment {
				| TopicPatternItem::Str(expected) => {
					if topic_index >= topic.segments.len() {
						return Err(TopicMatchError::UnexpectedEndOfTopic);
					}
					if topic.segments[topic_index] != *expected {
						return Err(TopicMatchError::SegmentMismatch {
							expected: expected.to_string(),
							found: topic.segments[topic_index].to_string(),
							position: topic_index,
						});
					}
					topic_index += 1;
				}
				| TopicPatternItem::Plus(opt_name) => {
					if topic_index >= topic.segments.len() {
						return Err(TopicMatchError::UnexpectedEndOfTopic);
					}
					let param_range = topic_index .. topic_index + 1;
					params.push(param_range.clone());
					topic_index += 1;
					if let Some(name) = opt_name {
						if named_params
							.iter()
							.any(|(existing_name, _)| existing_name == name)
						{
							return Err(
								TopicMatchError::DuplicateParameterName,
							);
						}
						named_params.push((name.clone(), param_range));
					}
				}
				| TopicPatternItem::Hash(opt_name) => {
					let param_range = topic_index .. topic.segments.len();
					params.push(param_range.clone());
					if let Some(name) = opt_name {
						if named_params
							.iter()
							.any(|(existing_name, _)| existing_name == name)
						{
							return Err(
								TopicMatchError::DuplicateParameterName,
							);
						}
						named_params.push((name.clone(), param_range));
					}
					if i < self.len() - 1 {
						return Err(TopicMatchError::UnexpectedHashSegment);
					}
					return Ok(TopicMatch::from_match_result(
						topic,
						params,
						named_params,
					));
				}
			}
		}
		if topic_index < topic.segments.len() {
			return Err(TopicMatchError::UnexpectedEndOfPattern);
		}
		Ok(TopicMatch::from_match_result(topic, params, named_params))
	}
}

// impl TryFrom<String> for TopicPatternPath {
// 	type Error = TopicPatternError;

// 	fn try_from(topic_pattern: String) -> Result<Self, Self::Error> {
// 		TopicPatternPath::new_from_string(topic_pattern)
// 	}
// }

// impl TryFrom<&[TopicPatternItem]> for TopicPatternPath {
// 	type Error = TopicPatternError;

// 	fn try_from(segments: &[TopicPatternItem]) -> Result<Self, Self::Error> {
// 		Self::new_from_segments(segments)
// 	}
// }

impl std::fmt::Display for TopicPatternPath {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// Convert segments to strings and join them with "/"
		let path = self.topic_pattern();
		write!(f, "{path}")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn str_to_topic_pattern_path(
		topic: &str,
	) -> Result<TopicPatternPath, TopicPatternError> {
		TopicPatternPath::new_from_string(topic, CacheStrategy::NoCache)
	}

	#[test]
	fn test_simple_string_pattern() {
		let result = str_to_topic_pattern_path("simple/path").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str(Substr::from("simple")),
				TopicPatternItem::Str(Substr::from("path"))
			]
		);
	}

	#[test]
	fn test_pattern_with_star() {
		let result = str_to_topic_pattern_path("devices/+/status").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str(Substr::from("devices")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("status"))
			]
		);
	}

	#[test]
	fn test_pattern_with_hash() {
		let result = str_to_topic_pattern_path("sensors/#").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str(Substr::from("sensors")),
				TopicPatternItem::Hash(None)
			]
		);
	}

	#[test]
	fn test_pattern_with_both_wildcards() {
		let result = str_to_topic_pattern_path("home/+/device/#").unwrap();
		assert_eq!(
			result.segments,
			vec![
				TopicPatternItem::Str(Substr::from("home")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("device")),
				TopicPatternItem::Hash(None)
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
		assert_eq!(result_star.segments, vec![TopicPatternItem::Plus(None)]);

		let result_hash = str_to_topic_pattern_path("#").unwrap();
		assert_eq!(result_hash.segments, vec![TopicPatternItem::Hash(None)]);
	}

	#[test]
	fn test_consecutive_separators() {
		let result = str_to_topic_pattern_path("topic//subtopic");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments,
			vec![
				TopicPatternItem::Str(Substr::from("topic")),
				TopicPatternItem::Str(Substr::from("")),
				TopicPatternItem::Str(Substr::from("subtopic"))
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
				TopicPatternItem::Str(Substr::from("")),
				TopicPatternItem::Str(Substr::from("start"))
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
				TopicPatternItem::Str(Substr::from("end")),
				TopicPatternItem::Str(Substr::from(""))
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
				TopicPatternItem::Str(Substr::from("пристрої")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("статус"))
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
				TopicPatternItem::Str(Substr::from("device-123")),
				TopicPatternItem::Str(Substr::from("status@home"))
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
		if let Err(err) = str_to_topic_pattern_path("") {
			assert_eq!(err, TopicPatternError::EmptyTopic);
		} else {
			panic!("Expected error for empty topic pattern");
		}

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
