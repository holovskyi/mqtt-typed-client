#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
use std::collections::HashMap;

use thiserror::Error;

use super::topic_matcher::{TopicMatcherError, TopicMatcherNode};
use super::topic_pattern_path::{TopicPatternError, TopicPatternPath};
use crate::topic::topic_match::TopicPath;

/// Errors that can occur during topic routing operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicRouterError {
	/// Topic pattern validation failed
	#[error("Invalid topic pattern: {0}")]
	InvalidPattern(#[from] TopicPatternError),

	/// Topic matching operation failed
	#[error("Topic matching failed: {0}")]
	MatchingFailed(#[from] TopicMatcherError),

	/// Subscription with given ID was not found
	#[error("Subscription {id:?} not found")]
	SubscriptionNotFound { id: SubscriptionId },

	/// Topic is invalid for routing operations
	#[error("Topic '{topic}' is invalid for routing: {reason}")]
	InvalidRoutingTopic { topic: String, reason: String },

	/// Internal state corruption detected
	#[error("Internal routing state corrupted: {details}")]
	InternalStateCorrupted { details: String },
}

impl TopicRouterError {
	/// Creates a new SubscriptionNotFound error
	pub fn subscription_not_found(id: SubscriptionId) -> Self {
		Self::SubscriptionNotFound { id }
	}

	/// Creates a new InvalidRoutingTopic error
	pub fn invalid_routing_topic(
		topic: impl Into<String>,
		reason: impl Into<String>,
	) -> Self {
		Self::InvalidRoutingTopic {
			topic: topic.into(),
			reason: reason.into(),
		}
	}

	/// Creates a new InternalStateCorrupted error
	pub fn internal_state_corrupted(details: impl Into<String>) -> Self {
		Self::InternalStateCorrupted {
			details: details.into(),
		}
	}

	/// Returns true if this error indicates a client-side problem
	pub fn is_client_error(&self) -> bool {
		match self {
			| TopicRouterError::InvalidPattern(_) => true,
			| TopicRouterError::MatchingFailed(_) => true,
			| TopicRouterError::SubscriptionNotFound { .. } => true,
			| TopicRouterError::InvalidRoutingTopic { .. } => true,
			| TopicRouterError::InternalStateCorrupted { .. } => false,
		}
	}

	/// Returns true if this error is recoverable through retry
	pub fn is_retryable(&self) -> bool {
		match self {
			| TopicRouterError::InvalidPattern(_) => false,
			| TopicRouterError::MatchingFailed(matcher_err) => {
				matcher_err.is_recoverable()
			}
			| TopicRouterError::SubscriptionNotFound { .. } => false,
			| TopicRouterError::InvalidRoutingTopic { .. } => false,
			| TopicRouterError::InternalStateCorrupted { .. } => false,
		}
	}

	/// Returns the error type for categorization
	pub fn error_type(&self) -> &'static str {
		match self {
			| TopicRouterError::InvalidPattern(_) => "invalid_pattern",
			| TopicRouterError::MatchingFailed(_) => "matching_failed",
			| TopicRouterError::SubscriptionNotFound { .. } => {
				"subscription_not_found"
			}
			| TopicRouterError::InvalidRoutingTopic { .. } => {
				"invalid_routing_topic"
			}
			| TopicRouterError::InternalStateCorrupted { .. } => {
				"internal_state_corrupted"
			}
		}
	}
}

/// A subscription identifier.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct SubscriptionId(usize);

type SubscriptionTable<T> = HashMap<SubscriptionId, T>;
//type RouteCallback = Box<dyn for<'a, 'b> Fn(&'a str, &'b [u8]) + Send + Sync>;

pub struct TopicRouter<T> {
	topic_matcher: TopicMatcherNode<SubscriptionTable<T>>,
	subscriptions: SubscriptionTable<TopicPatternPath>,
	next_id: usize,
}

impl<T> TopicRouter<T> {
	pub fn new() -> Self {
		Self {
			topic_matcher: TopicMatcherNode::new(),
			subscriptions: SubscriptionTable::new(),
			next_id: 0,
		}
	}

	pub fn subscribe(
		&mut self,
		topic: TopicPatternPath,
		subscription: T,
	) -> (bool, SubscriptionId) {
		let id = SubscriptionId(self.next_id);
		self.next_id = self.next_id.wrapping_add(1);

		let subscription_table =
			self.topic_matcher.subscribe_to_pattern(&topic);
		let is_empty = subscription_table.is_empty();
		subscription_table.insert(id, subscription);
		self.subscriptions.insert(id, topic);
		(is_empty, id)
	}

	pub fn unsubscribe(
		&mut self,
		id: &SubscriptionId,
	) -> Result<(bool, TopicPatternPath), TopicRouterError> {
		let topic = self.subscriptions.remove(id);
		match topic {
			| Some(topic) => {
				let topic_now_empty =
					self.topic_matcher.update_node(topic.slice(), |table| {
						table.remove(id);
					})?;
				Ok((topic_now_empty, topic))
			}
			| None => Err(TopicRouterError::subscription_not_found(*id)),
		}
	}

	pub fn get_subscribers<'a>(
		&'a self,
		topic: &TopicPath,
	) -> Vec<(&'a SubscriptionId, &'a TopicPatternPath, &'a T)> {
		let subscribers = self.topic_matcher.find_by_path(topic);
		subscribers
			.into_iter()
			.flat_map(|hash_map| hash_map.iter())
			.map(|(id, subscription)| {
				let topic_pattern = self
					.subscriptions
					.get(id)
					.expect("Subscription ID should exist in subscriptions");
				(id, topic_pattern, subscription)
			})
			.collect()
	}

	pub fn get_active_subscriptions(
		&self,
	) -> impl Iterator<Item = &TopicPatternPath> {
		self.subscriptions.values()
	}

	/// Cleanup all internal data structures and close subscriber channels
	/// This method is called during shutdown to ensure proper resource cleanup
	pub fn cleanup(&mut self) {
		// Replacing topic_matcher with new instance triggers Drop for all subscription channels
		// This ensures all subscribers receive a channel close signal
		self.topic_matcher = TopicMatcherNode::new();
		self.subscriptions.clear();
		self.next_id = 0;
	}

	pub fn get_topic_by_id(
		&self,
		id: &SubscriptionId,
	) -> Result<&TopicPatternPath, TopicRouterError> {
		self.subscriptions
			.get(id)
			.ok_or(TopicRouterError::subscription_not_found(*id))
	}
}
