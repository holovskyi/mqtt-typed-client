use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::{self, Display, Write};
use std::slice::Iter;
use std::sync::Arc;
use std::sync::Mutex;

use arcstr::ArcStr;
use lru::LruCache;
use smallvec::SmallVec;
use thiserror::Error;

use super::topic_pattern_item::{TopicPatternError, TopicPatternItem};
use crate::routing::subscription_manager::CacheStrategy;
use crate::topic::topic_match::{TopicMatch, TopicMatchError, TopicPath};

/// Error types for formatting topics with parameters
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicFormatError {
	/// Attempted to format a topic with a hash wildcard (#) which is not allowed
	#[error("Cannot format topic with # wildcard for publishing")]
	HashWildcardNotSupported,

	/// Parameter count mismatch when formatting a topic
	#[error(
		"Parameter count mismatch: expected {expected}, provided {provided}"
	)]
	ParameterCountMismatch {
		/// Expected number of parameters
		expected: usize,
		/// Number of parameters actually provided
		provided: usize,
	},
	/// Error during formatting, e.g. invalid parameter type
	#[error("Error formatting topic")]
	FormatError {
		#[source]
		/// The underlying formatting error
		source: fmt::Error,
	},
}

impl From<fmt::Error> for TopicFormatError {
	fn from(source: fmt::Error) -> Self {
		TopicFormatError::FormatError { source }
	}
}

/// Parsed MQTT topic pattern with wildcard support
#[derive(Debug)]
pub struct TopicPatternPath {
	template_pattern: ArcStr, // original topic pattern as a string
	mqtt_topic_subscription: ArcStr, // mqtt topic pattern with wildcards "sensors/+/data"
	segments: Vec<TopicPatternItem>,
	/// Optional LRU cache for topic match results.
	///
	/// Uses `Mutex` instead of `RefCell` for interior mutability because:
	/// 1. This struct needs to be `Send + Sync` to work in the actor-based subscription manager
	/// 2. Although used in single-threaded actor context, `RefCell` is not `Send`
	/// 3. No contention occurs since access is serialized within the actor's event loop
	/// 4. `Mutex` provides the same interior mutability as `RefCell` but with `Send + Sync`
	match_cache: Option<Mutex<LruCache<ArcStr, Arc<TopicMatch>>>>,

	parameter_bindings: Option<SmallVec<[(ArcStr, ArcStr); 4]>>,
}

impl Clone for TopicPatternPath {
	fn clone(&self) -> Self {
		Self {
			template_pattern: self.template_pattern.clone(),
			mqtt_topic_subscription: self.mqtt_topic_subscription.clone(),
			segments: self.segments.clone(),
			match_cache: self.match_cache.as_ref().map(|cache| {
				let cache_guard = cache.lock().unwrap();
				let capacity = cache_guard.cap();
				drop(cache_guard);
				Mutex::new(LruCache::new(capacity))
			}),
			parameter_bindings: self.parameter_bindings.clone(),
		}
	}
}

impl TopicPatternPath {
	/// Creates a topic pattern from string with optional caching.
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

		//Error on duplicate named parameters
		let mut seen_names = HashSet::new();
		for segment in &segments {
			if let Some(name) = segment.param_name() {
				if !seen_names.insert(name.to_string()) {
					return Err(TopicPatternError::wildcard_usage(
						segment.as_str(),
					));
				}
			}
		}

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
			template_pattern: topic_pattern,
			mqtt_topic_subscription: ArcStr::from(
				Self::to_mqtt_subscription_pattern(&segments),
			),
			segments,
			match_cache,
			parameter_bindings: None,
		})
	}

	/// Get the cache strategy of this topic pattern.
	pub fn cache_strategy(&self) -> CacheStrategy {
		match &self.match_cache {
			| Some(cache_mutex) => {
				let cache_guard = cache_mutex.lock().unwrap();
				CacheStrategy::Lru(cache_guard.cap())
			}
			| None => CacheStrategy::NoCache,
		}
	}

	#[cfg(test)]
	/// Creates a topic pattern from segments directly, useful for testing.
	pub fn new_from_segments(
		segments: &[TopicPatternItem],
	) -> Result<Self, TopicPatternError> {
		let topic_pattern = ArcStr::from(Self::to_template_pattern(segments));
		let pattern = Self {
			mqtt_topic_subscription: ArcStr::from(
				Self::to_mqtt_subscription_pattern(segments),
			),
			template_pattern: topic_pattern.clone(),
			segments: segments.to_vec(),
			match_cache: None,
			parameter_bindings: None,
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

	/// Returns MQTT pattern with wildcards for broker subscription.
	pub fn mqtt_pattern(&self) -> ArcStr {
		match &self.parameter_bindings {
			| Some(bindings) => self.generate_concrete_mqtt_pattern(bindings),
			| None => self.mqtt_topic_subscription.clone(),
		}
	}

	fn generate_concrete_mqtt_pattern(
		&self,
		bindings: &[(ArcStr, ArcStr)],
	) -> ArcStr {
		let mut new_segments = self.segments.clone();
		let mut is_applied = false;

		for (param_name, value) in bindings {
			if let Some(segment_pos) = new_segments.iter().position(|segment| {
                matches!(segment, TopicPatternItem::Plus(Some(name)) if name == param_name)
            }) {
                new_segments[segment_pos] = TopicPatternItem::Str(value.into());
				is_applied = true;
            } else {
				tracing::debug!(
				pattern = %self.topic_pattern(),
				"Parameter '{param_name}' not found in pattern"
				);
               panic!("Parameter '{param_name}' not found in pattern" );
            }
		}
		if !is_applied {
			tracing::debug!(
				pattern = %self.topic_pattern(),
				"with_parameters() called with no applicable parameters - returning original pattern unchanged"
			);
			return self.mqtt_topic_subscription.clone();
		}
		ArcStr::from(Self::to_mqtt_subscription_pattern(&new_segments))
	}

	/// Returns original pattern with named parameters.
	pub fn topic_pattern(&self) -> ArcStr {
		self.template_pattern.clone()
	}

	/// Returns true if pattern has no segments.
	pub fn is_empty(&self) -> bool {
		self.segments.is_empty()
	}

	/// Returns true if pattern contains multi-level wildcard (#).
	pub fn contains_hash(&self) -> bool {
		self.segments
			.last()
			.is_some_and(|s| matches!(s, TopicPatternItem::Hash(_)))
	}

	/// Returns iterator over pattern segments.
	pub fn iter(&self) -> Iter<TopicPatternItem> {
		self.segments.iter()
	}

	/// Returns number of segments in pattern.
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

	/// Returns pattern segments as slice.
	pub fn slice(&self) -> &[TopicPatternItem] {
		&self.segments
	}

	/// Returns pattern segments for testing.
	#[cfg(test)]
	pub fn segments(&self) -> &Vec<TopicPatternItem> {
		&self.segments
	}

	/// Formats topic by substituting wildcards with provided parameters
	pub fn format_topic(
		&self,
		params: &[&dyn Display],
	) -> Result<String, TopicFormatError> {
		let wildcard_count =
			self.segments.iter().filter(|s| s.is_wildcard()).count();

		if params.len() != wildcard_count {
			return Err(TopicFormatError::ParameterCountMismatch {
				expected: wildcard_count,
				provided: params.len(),
			});
		}

		let mut result = String::with_capacity(self.topic_pattern().len() + 10); //Est.
		let mut param_index = 0;

		for (i, segment) in self.segments.iter().enumerate() {
			if i > 0 {
				result.push('/');
			}

			match segment {
				| TopicPatternItem::Str(s) => result.push_str(s),
				| TopicPatternItem::Plus(_) => {
					write!(result, "{}", params[param_index])?;
					param_index += 1;
				}
				| TopicPatternItem::Hash(_) => {
					return Err(TopicFormatError::HashWildcardNotSupported);
				}
			}
		}

		Ok(result)
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

	#[cfg(test)]
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

	/// Checks if the provided topic pattern is compatible with this one.
	///
	/// Static segments can differ, but wildcards must be identical in type,
	/// order, and names (if named).
	pub fn check_pattern_compatibility(
		&self,
		custom_topic: impl TryInto<TopicPatternPath, Error: Into<TopicPatternError>>,
	) -> Result<Self, TopicPatternError> {
		let candidate = custom_topic.try_into().map_err(Into::into)?;
		// Validate wildcard structure compatibility
		let self_wildcards =
			self.segments.iter().filter(|item| item.is_wildcard());
		let candidate_wildcards =
			candidate.segments.iter().filter(|item| item.is_wildcard());

		if !self_wildcards.eq(candidate_wildcards) {
			return Err(TopicPatternError::pattern_mismatch(
				self.template_pattern.as_str(),
				candidate.template_pattern.as_str(),
			));
		}

		Ok(candidate)
	}

	/// Create new pattern with different cache strategy
	pub fn with_cache_strategy(&self, new_cache: CacheStrategy) -> Self {
		let mut new_pattern =
			Self::new_from_string(self.template_pattern.clone(), new_cache)
				.expect("Pattern already validated");
		new_pattern.parameter_bindings = self.parameter_bindings.clone();
		new_pattern
	}

	/// Add value for topic wildcard parameter
	pub fn bind_parameter(
		mut self,
		param_name: impl Into<ArcStr>,
		value: impl Into<ArcStr>,
	) -> Result<Self, TopicPatternError> {
		let param_name_arc = param_name.into();

		let param_exists = self.segments.iter().any(|segment| {
			matches!(segment, TopicPatternItem::Plus(Some(name)) if name.as_str() == param_name_arc.as_str())
		});
		if !param_exists {
			return Err(TopicPatternError::wildcard_usage(
				format!("Parameter '{param_name_arc}' not found in pattern '{}'", self.topic_pattern())
			));
		}
		
		let value_arc = value.into();

		let bindings =
			self.parameter_bindings.get_or_insert_with(SmallVec::new);

		if let Some(pos) =
			bindings.iter().position(|(k, _)| k == &param_name_arc)
		{
			bindings[pos].1 = value_arc;
		} else {
			bindings.push((param_name_arc, value_arc));
		}

		Ok(self)
	}

	/// Matches topic path against this pattern, extracting parameters.
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
						named_params.push((name.clone(), param_range));
					}
				}
				| TopicPatternItem::Hash(opt_name) => {
					let param_range = topic_index .. topic.segments.len();
					params.push(param_range.clone());
					if let Some(name) = opt_name {
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

impl std::fmt::Display for TopicPatternPath {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// Convert segments to strings and join them with "/"
		let path = self.topic_pattern();
		write!(f, "{path}")
	}
}

impl TryFrom<String> for TopicPatternPath {
	type Error = TopicPatternError;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		Self::new_from_string(value, CacheStrategy::NoCache)
	}
}

impl TryFrom<&str> for TopicPatternPath {
	type Error = TopicPatternError;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		Self::new_from_string(value, CacheStrategy::NoCache)
	}
}

impl TryFrom<ArcStr> for TopicPatternPath {
	type Error = TopicPatternError;

	fn try_from(value: ArcStr) -> Result<Self, Self::Error> {
		Self::new_from_string(value, CacheStrategy::NoCache)
	}
}
