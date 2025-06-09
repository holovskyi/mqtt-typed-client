use std::collections::{HashMap, HashSet};

use thiserror::Error;

use super::topic_pattern_path::{TopicPatternItem, TopicPatternPath};

/// Errors that can occur during topic matching operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicMatcherError {
	/// Topic path provided for matching is empty
	#[error("Topic path cannot be empty for matching")]
	EmptyTopicPath,

	/// Invalid topic segment encountered during matching
	#[error("Invalid topic segment '{segment}' at position {position}")]
	InvalidSegment { segment: String, position: usize },

	/// Topic path contains invalid UTF-8 characters
	#[error("Topic path contains invalid UTF-8: {details}")]
	InvalidUtf8 { details: String },
}

impl TopicMatcherError {
	/// Creates a new InvalidSegment error
	pub fn invalid_segment(
		segment: impl Into<String>,
		position: usize,
	) -> Self {
		Self::InvalidSegment {
			segment: segment.into(),
			position,
		}
	}

	/// Creates a new InvalidUtf8 error
	pub fn invalid_utf8(details: impl Into<String>) -> Self {
		Self::InvalidUtf8 {
			details: details.into(),
		}
	}

	/// Returns true if this error is recoverable
	pub fn is_recoverable(&self) -> bool {
		match self {
			| TopicMatcherError::EmptyTopicPath => false,
			| TopicMatcherError::InvalidSegment { .. } => true,
			| TopicMatcherError::InvalidUtf8 { .. } => false,
		}
	}

	/// Returns the error type for categorization
	pub fn error_type(&self) -> &'static str {
		match self {
			| TopicMatcherError::EmptyTopicPath => "empty_topic_path",
			| TopicMatcherError::InvalidSegment { .. } => "invalid_segment",
			| TopicMatcherError::InvalidUtf8 { .. } => "invalid_utf8",
		}
	}
}

/// Node in the topic matching tree that represents a part of the topic path.
/// Used internally by the `TopicMatcher`.
#[derive(Debug)]
pub struct TopicMatcherNode<T> {
	/// Data for exact topic segment match
	exact_match_data: Option<T>,

	/// Children nodes for exact matches of next segment
	exact_children: HashMap<String, TopicMatcherNode<T>>,

	/// Node for '+' pattern wildcard match (single segment)
	plus_wildcard_node: Option<Box<TopicMatcherNode<T>>>,

	/// Data for '#' pattern wildcard match (multiple segments)
	hash_wildcard_data: Option<T>,
}

pub trait IsEmpty {
	fn is_empty(&self) -> bool;
}

pub trait Len {
	fn len(&self) -> usize;
}

impl<T> IsEmpty for HashSet<T> {
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}

impl<T> Len for HashSet<T> {
	fn len(&self) -> usize {
		self.len()
	}
}

impl<K, V> IsEmpty for HashMap<K, V> {
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}

impl<K, V> Len for HashMap<K, V> {
	fn len(&self) -> usize {
		self.len()
	}
}

impl<T: Default + IsEmpty + Len> TopicMatcherNode<T> {
	/// Creates a new empty topic matcher node
	pub fn new() -> Self {
		Self {
			exact_match_data: None,
			exact_children: HashMap::new(),
			plus_wildcard_node: None,
			hash_wildcard_data: None,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.exact_match_data.as_ref().is_none_or(T::is_empty)
			&& self.exact_children.is_empty()
			&& self.plus_wildcard_node.is_none()
			&& self.hash_wildcard_data.as_ref().is_none_or(T::is_empty)
	}
	/// Finds or creates a subscription data entry matching the given topic pattern
	pub fn subscribe_to_pattern(
		&mut self,
		topic_path: &TopicPatternPath,
	) -> &mut T {
		let mut current_node = self;

		for segment in topic_path.iter() {
			match segment {
				| TopicPatternItem::Str(s) => {
					current_node = current_node
						.exact_children
						.entry(s.to_string())
						.or_insert_with(TopicMatcherNode::new)
				}
				| TopicPatternItem::Plus => {
					current_node =
						current_node.plus_wildcard_node.get_or_insert_with(
							|| Box::new(TopicMatcherNode::new()),
						)
				}
				| TopicPatternItem::Hash => {
					// Hash wildcard must be the last segment, so we can return immediately
					return current_node
						.hash_wildcard_data
						.get_or_insert_with(T::default);
				}
			}
		}
		current_node.exact_match_data.get_or_insert_with(T::default)
	}

	/// Finds or creates a subscription data entry matching the given topic pattern
	pub fn update_node<F>(
		&mut self,
		topic_path: &[TopicPatternItem],
		mut f: F,
	) -> Result<bool, TopicMatcherError>
	where
		F: FnMut(&mut T),
	{
		if topic_path.is_empty() {
			let data = self.exact_match_data.as_mut().ok_or_else(|| {
				TopicMatcherError::invalid_segment(
					"no_data_for_empty_path".to_string(),
					0,
				)
			})?;
			f(data);
			if data.is_empty() {
				self.exact_match_data = None
			}
			return Ok(self.is_empty());
		}
		let current_segment = &topic_path[0];
		let rest_segments = &topic_path[1 ..];

		match current_segment {
			| TopicPatternItem::Str(s) => {
				let child_node =
					self.exact_children.get_mut(s).ok_or_else(|| {
						TopicMatcherError::invalid_segment(s.clone(), 0)
					})?;
				if child_node.update_node(rest_segments, f)? {
					self.exact_children.remove(s);
					return Ok(self.is_empty());
				}
			}
			| TopicPatternItem::Plus => {
				let child_node =
					self.plus_wildcard_node.as_mut().ok_or_else(|| {
						TopicMatcherError::invalid_segment("+".to_string(), 0)
					})?;
				if child_node.update_node(rest_segments, f)? {
					self.plus_wildcard_node = None;
					return Ok(self.is_empty());
				}
			}
			| TopicPatternItem::Hash => {
				let hash_wildcard_data =
					self.hash_wildcard_data.as_mut().ok_or_else(|| {
						TopicMatcherError::invalid_segment("#".to_string(), 0)
					})?;
				f(hash_wildcard_data);
				if hash_wildcard_data.is_empty() {
					self.hash_wildcard_data = None;
					return Ok(self.is_empty());
				}
			}
		}
		Ok(false)
	}

	/// Recursively collects all subscription data that matches the given topic path segments
	fn collect_matching_subscriptions<'a>(
		&'a self,
		path_segments: &[&str],
		matching_data: &mut Vec<&'a T>,
	) {
		match path_segments {
			| [] => {
				// At end of path, collect data from this node if present
				self.exact_match_data
					.iter()
					.for_each(|data| matching_data.push(data));
				self.hash_wildcard_data
					.iter()
					.for_each(|data| matching_data.push(data))
			}
			| [segment, remaining_segments @ ..] => {
				// Check for exact segment match
				if let Some(child) = self.exact_children.get(*segment) {
					child.collect_matching_subscriptions(
						remaining_segments,
						matching_data,
					);
				}
				// Check for + wildcard match (matches any single segment)
				self.plus_wildcard_node.iter().for_each(|plus_node| {
					plus_node.collect_matching_subscriptions(
						remaining_segments,
						matching_data,
					)
				});
				// # wildcard matches remainder of path
				self.hash_wildcard_data
					.iter()
					.for_each(|hash_data| matching_data.push(hash_data));
			}
		}
	}

	/// Finds all subscription data entries matching the given topic path
	pub fn find_by_path<'a>(&'a self, path: &str) -> Vec<&'a T> {
		let path_segments: Vec<&str> = path.split('/').collect();
		let mut matching_subscribers = Vec::new();
		self.collect_matching_subscriptions(
			&path_segments,
			&mut matching_subscribers,
		);
		matching_subscribers
	}

	fn collect_active_subscriptions<'a>(
		&'a self,
		current_path: &[TopicPatternItem],
		result: &mut Vec<(TopicPatternPath, &'a T)>,
	) {
		self.exact_match_data.iter().for_each(|data| {
			let path = TopicPatternPath::try_from(current_path)
				.expect("Internal path should always be valid");
			result.push((path, data))
		});
		self.hash_wildcard_data.iter().for_each(|data| {
			let mut segments = TopicPatternPath::try_from(current_path)
				.expect("Internal path should always be valid");
			segments
				.push(TopicPatternItem::Hash)
				.expect("Adding hash to internal path should always succeed");
			result.push((segments, data))
		});
		self.plus_wildcard_node.iter().for_each(|plus_node| {
			let mut segments = current_path.to_vec();
			segments.push(TopicPatternItem::Plus);
			plus_node.collect_active_subscriptions(&segments, result);
		});
		for (exact_segment, child) in &self.exact_children {
			let mut segments = current_path.to_vec();
			segments.push(TopicPatternItem::Str(exact_segment.clone()));
			child.collect_active_subscriptions(&segments, result);
		}
	}

	pub fn active_subscriptions(&self) -> Vec<(TopicPatternPath, &T)> {
		let mut result = Vec::new();
		self.collect_active_subscriptions(&[], &mut result);
		result
	}
}
