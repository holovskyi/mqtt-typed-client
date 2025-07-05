#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
use std::collections::{HashMap, HashSet};

use arcstr::Substr;
use thiserror::Error;

use super::topic_pattern_path::TopicPatternPath;
use super::topic_pattern_item::TopicPatternItem;
use crate::topic::topic_match::TopicPath;

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
}

/// Node in the topic matching tree that represents a part of the topic path.
/// Used internally by the `TopicMatcher`.
#[derive(Debug)]
pub struct TopicMatcherNode<T> {
	/// Data for exact topic segment match
	exact_match_data: Option<T>,

	/// Children nodes for exact matches of next segment
	exact_children: HashMap<Substr, TopicMatcherNode<T>>,

	/// Node for '+' pattern wildcard match (single segment)
	single_level_wildcard_node: Option<Box<TopicMatcherNode<T>>>,

	/// Data for '#' pattern wildcard match (multiple segments)
	multi_level_wildcard_data: Option<T>,
}

pub trait Len {
	fn len(&self) -> usize;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl<T> Len for HashSet<T> {
	fn len(&self) -> usize {
		self.len()
	}
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}

impl<K, V> Len for HashMap<K, V> {
	fn len(&self) -> usize {
		self.len()
	}
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}

impl<T: Default + Len> Default for TopicMatcherNode<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Default + Len> TopicMatcherNode<T> {
	/// Creates a new empty topic matcher node
	pub fn new() -> Self {
		Self {
			exact_match_data: None,
			exact_children: HashMap::new(),
			single_level_wildcard_node: None,
			multi_level_wildcard_data: None,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.exact_match_data.as_ref().is_none_or(T::is_empty)
			&& self.exact_children.is_empty()
			&& self.single_level_wildcard_node.is_none()
			&& self
				.multi_level_wildcard_data
				.as_ref()
				.is_none_or(T::is_empty)
	}
	/// Finds or creates a subscription data entry matching the given topic pattern
	pub fn get_or_create_subscription_table(
		&mut self,
		topic_path: &TopicPatternPath,
	) -> &mut T {
		let mut current_node = self;

		for segment in topic_path.iter() {
			match segment {
				| TopicPatternItem::Str(s) => {
					current_node = current_node
						.exact_children
						.entry(s.clone())
						.or_default()
				}
				| TopicPatternItem::Plus(param_name) => {
					match topic_path.get_bound_value(param_name.as_deref()) {
						Some(bound_value) => {
							current_node = current_node
								.exact_children
								.entry(bound_value.clone().into())
								.or_default()
						}
						None => {
							current_node = current_node
								.single_level_wildcard_node
								.get_or_insert_with(|| Box::new(TopicMatcherNode::new()))
						}
					}
				}
				| TopicPatternItem::Hash(_) => {
					// Hash wildcard must be the last segment, so we can return immediately
					return current_node
						.multi_level_wildcard_data
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
						TopicMatcherError::invalid_segment(s.as_str(), 0)
					})?;
				if child_node.update_node(rest_segments, f)? {
					self.exact_children.remove(s);
					return Ok(self.is_empty());
				}
			}
			| TopicPatternItem::Plus(_) => {
				let child_node = self
					.single_level_wildcard_node
					.as_mut()
					.ok_or_else(|| {
						TopicMatcherError::invalid_segment("+".to_string(), 0)
					})?;
				if child_node.update_node(rest_segments, f)? {
					self.single_level_wildcard_node = None;
					return Ok(self.is_empty());
				}
			}
			| TopicPatternItem::Hash(_) => {
				let hash_wildcard_data = self
					.multi_level_wildcard_data
					.as_mut()
					.ok_or_else(|| {
						TopicMatcherError::invalid_segment("#".to_string(), 0)
					})?;
				f(hash_wildcard_data);
				if hash_wildcard_data.is_empty() {
					self.multi_level_wildcard_data = None;
					return Ok(self.is_empty());
				}
			}
		}
		Ok(false)
	}

	/// Recursively collects all subscription data that matches the given topic path segments
	fn collect_matching_subscriptions<'a>(
		&'a self,
		topic: &[Substr],
		matching_data: &mut Vec<&'a T>,
	) {
		match topic {
			| [] => {
				// At end of path, collect data from this node if present
				self.exact_match_data
					.iter()
					.for_each(|data| matching_data.push(data));
				self.multi_level_wildcard_data
					.iter()
					.for_each(|data| matching_data.push(data))
			}
			| [segment, remaining_segments @ ..] => {
				// Check for exact segment match
				if let Some(child) = self.exact_children.get(segment) {
					child.collect_matching_subscriptions(
						remaining_segments,
						matching_data,
					);
				}
				// Check for + wildcard match (matches any single segment)
				self.single_level_wildcard_node
					.iter()
					.for_each(|plus_node| {
						plus_node.collect_matching_subscriptions(
							remaining_segments,
							matching_data,
						)
					});
				// # wildcard matches remainder of path
				self.multi_level_wildcard_data
					.iter()
					.for_each(|hash_data| matching_data.push(hash_data));
			}
		}
	}

	/// Finds all subscription data entries matching the given topic path
	pub fn find_by_path<'a>(&'a self, topic: &TopicPath) -> Vec<&'a T> {
		//let path_segments: Vec<&str> = path.split('/').collect();
		let mut matching_subscribers = Vec::new();
		self.collect_matching_subscriptions(
			&topic.segments,
			&mut matching_subscribers,
		);
		matching_subscribers
	}

	#[cfg(test)]
	// NOTE: These methods are only available in test builds and are used for
	// testing the tree traversal logic. In production, use TopicRouter::get_active_subscriptions()
	// which is more efficient.
	fn collect_active_subscriptions_internal<'a>(
		&'a self,
		current_path: &mut Vec<TopicPatternItem>,
		result: &mut Vec<(TopicPatternPath, &'a T)>,
	) {
		// Collect exact match data if present
		if let Some(data) = &self.exact_match_data {
			let path =
				TopicPatternPath::new_from_segments(current_path.as_slice())
					.expect("Internal path should always be valid");
			result.push((path, data))
		};
		// Collect hash wildcard data if present
		if let Some(data) = &self.multi_level_wildcard_data {
			current_path.push(TopicPatternItem::Hash(None));
			let topic_path =
				TopicPatternPath::new_from_segments(current_path.as_slice())
					.expect("Internal path should always be valid");
			result.push((topic_path, data));
			current_path.pop();
		};
		if let Some(plus_node) = &self.single_level_wildcard_node {
			current_path.push(TopicPatternItem::Plus(None));
			plus_node.collect_active_subscriptions_internal(current_path, result);
			current_path.pop();
		};
		for (exact_segment, child) in &self.exact_children {
			current_path.push(TopicPatternItem::Str(exact_segment.clone()));
			child.collect_active_subscriptions_internal(current_path, result);
			current_path.pop();
		}
	}

	#[cfg(test)]
	pub fn collect_active_subscriptions(&self) -> Vec<(TopicPatternPath, &T)> {
		let mut result = Vec::new();
		self.collect_active_subscriptions_internal(&mut Vec::new(), &mut result);
		result
	}
}
