//! Subscription router built on the topic matcher.
//!
//! [`TopicRouter`] maps MQTT subscription patterns to caller-supplied payloads
//! `T`, assigns each a [`SubscriptionId`], and resolves all payloads whose
//! pattern matches a delivered topic. It also tracks the set of distinct broker
//! subscriptions (with their effective QoS) so callers know when to actually
//! subscribe or unsubscribe on the wire.

#![allow(clippy::missing_docs_in_private_items)]
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use arcstr::ArcStr;
use thiserror::Error;

use crate::qos::QoS;
use crate::topic_match::TopicPath;
use crate::topic_matcher::{TopicMatcherError, TopicMatcherNode};
use crate::topic_pattern_item::TopicPatternError;
use crate::topic_pattern_path::TopicPatternPath;

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
	SubscriptionNotFound {
		/// The subscription id that could not be found.
		id: SubscriptionId,
	},

	/// Topic is invalid for routing operations
	#[error("Topic '{topic}' is invalid for routing: {reason}")]
	InvalidRoutingTopic {
		/// The topic that was rejected.
		topic: String,
		/// Why the topic is invalid for routing.
		reason: String,
	},

	/// Internal state corruption detected
	#[error("Internal routing state corrupted: {details}")]
	InternalStateCorrupted {
		/// Description of the detected inconsistency.
		details: String,
	},
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
}

/// A subscription identifier.
///
/// Used for tracking individual subscriptions and handling cancellation errors.
/// Primarily useful for advanced error handling and debugging.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct SubscriptionId(usize);

impl Display for SubscriptionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "SubscriptionId({})", self.0)
	}
}

type SubscriptionTable<T> = HashMap<SubscriptionId, T>;
//type RouteCallback = Box<dyn for<'a, 'b> Fn(&'a str, &'b [u8]) + Send + Sync>;

/// Routes MQTT topics to subscription payloads.
///
/// Each subscription pattern is associated with a payload `T` and a unique
/// [`SubscriptionId`]. Delivered topics are matched against all stored patterns
/// (including `+`/`#` wildcards); the router also bookkeeps which distinct
/// broker subscriptions are active and at what QoS.
pub struct TopicRouter<T> {
	topic_matcher: TopicMatcherNode<SubscriptionTable<T>>,
	subscriptions: SubscriptionTable<(TopicPatternPath, QoS)>,
	next_id: usize,
}

impl<T> Default for TopicRouter<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> TopicRouter<T> {
	/// Creates an empty router with no subscriptions.
	pub fn new() -> Self {
		Self {
			topic_matcher: TopicMatcherNode::new(),
			subscriptions: SubscriptionTable::new(),
			next_id: 0,
		}
	}

	/// Registers a subscription for `topic` with the given `qos` and payload.
	///
	/// Returns `(needs_subscribe, id)`: `needs_subscribe` is `true` when this is
	/// the first subscription for the pattern or it raises the effective QoS, so
	/// the caller must (re)subscribe on the broker; `id` identifies the new
	/// subscription for later [`unsubscribe`](Self::unsubscribe).
	pub fn add_subscription(
		&mut self,
		topic: TopicPatternPath,
		qos: impl Into<QoS>,
		subscription: T,
	) -> (bool, SubscriptionId) {
		let qos = qos.into();
		let subscription_table =
			self.topic_matcher.get_or_create_subscription_table(&topic);
		let needs_subscribe = subscription_table
			.keys()
			.map(|id| {
				self.subscriptions
					.get(id)
					.unwrap_or_else(|| {
						panic!(
							"BUG: Subscription ID {id:?} exists in topic \
							 matcher but missing from subscriptions. Topic: \
							 {topic}"
						)
					})
					.1
			})
			.max_by_key(|qos| *qos as u8)
			.is_none_or(|max| qos > max);

		let id = SubscriptionId(self.next_id);
		self.next_id = self.next_id.wrapping_add(1);

		subscription_table.insert(id, subscription);
		self.subscriptions.insert(id, (topic, qos));

		(needs_subscribe, id)
	}

	/// Removes the subscription identified by `id`.
	///
	/// Returns `(topic_now_empty, pattern)`: `topic_now_empty` is `true` when no
	/// other subscription remains for the pattern, so the caller should
	/// unsubscribe it on the broker. Fails with
	/// [`TopicRouterError::SubscriptionNotFound`] if `id` is unknown.
	pub fn unsubscribe(
		&mut self,
		id: &SubscriptionId,
	) -> Result<(bool, TopicPatternPath), TopicRouterError> {
		// TODO(enhancement): Implement QoS downgrade.
		// We currently keep the higher QoS on the broker after a removal (safe
		// but potentially wasteful). To implement: after removing this id,
		// recompute the max QoS among the remaining subscribers for the pattern
		// — `get_max_qos_for_topic` below is the ready-made helper for that — and
		// mirror the QoS-raising logic in `add_subscription`.
		let topic = self.subscriptions.remove(id);
		match topic {
			| Some((topic_pattern, _qos)) => {
				let resolved_segments = topic_pattern.resolve_bound_segments();
				let topic_now_empty = self.topic_matcher.update_node(
					&resolved_segments,
					|table| {
						table.remove(id);
					},
				)?;
				Ok((topic_now_empty, topic_pattern))
			}
			| None => Err(TopicRouterError::subscription_not_found(*id)),
		}
	}

	/// Returns every subscription whose pattern matches `topic`.
	///
	/// Each entry is `(id, (pattern, qos), payload)` for a matching subscription
	/// (one delivered topic may match several patterns via `+`/`#` wildcards).
	pub fn get_subscribers<'a>(
		&'a self,
		topic: &TopicPath,
	) -> Vec<(&'a SubscriptionId, &'a (TopicPatternPath, QoS), &'a T)> {
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

	/// Iterates over every active subscription as `(pattern, qos)`.
	///
	/// Patterns are not deduplicated; multiple subscriptions may share one.
	pub fn get_active_subscriptions(
		&self,
	) -> impl Iterator<Item = &(TopicPatternPath, QoS)> {
		self.subscriptions.values()
	}

	/// Finds the maximum QoS among the subscribers to a single topic pattern.
	///
	/// Intentionally unused for now: it is the ready-made helper for the QoS
	/// downgrade described in the TODO inside [`unsubscribe`](Self::unsubscribe).
	/// The QoS-raising counterpart is currently inlined in
	/// [`add_subscription`](Self::add_subscription); when downgrade is
	/// implemented, the two should share this helper.
	#[allow(dead_code)] // Kept as the helper for the planned QoS-downgrade in unsubscribe()
	fn get_max_qos_for_topic(
		&self,
		topic: &TopicPatternPath,
		topic_subscriptions: &HashMap<SubscriptionId, T>,
	) -> QoS {
		debug_assert!(
			!topic_subscriptions.is_empty(),
			"topic_subscriptions should never be empty - this is guaranteed \
			 by collect_active_subscriptions()"
		);

		let max_qos = topic_subscriptions
			.keys()
			.map(|id| {
				self.subscriptions
					.get(id)
					.unwrap_or_else(|| {
						panic!(
							"BUG: Subscription ID {id:?} exists in topic \
							 matcher but missing from subscriptions. Topic: \
							 {topic}"
						)
					})
					.1
			})
			.max_by_key(|qos| *qos as u8)
			.unwrap();
		max_qos
	}

	/// Get all unique active topic patterns
	pub fn get_topics_for_unsubscribe(&self) -> HashSet<ArcStr> {
		self.subscriptions
			.values()
			.map(|(topic, _)| topic.mqtt_pattern())
			.collect()
	}

	/// Get all active topic patterns with their maximum QoS
	/// Returns unique topics (grouped by pattern) with the highest QoS among all subscribers
	pub fn get_topics_for_resubscribe(&self) -> HashMap<ArcStr, QoS> {
		let mut result: HashMap<ArcStr, QoS> = HashMap::new();

		for (topic, qos) in self.subscriptions.values() {
			let mqtt_pattern = topic.mqtt_pattern();
			result
				.entry(mqtt_pattern)
				.and_modify(|existing_qos| {
					if *qos > *existing_qos {
						*existing_qos = *qos;
					}
				})
				.or_insert(*qos);
		}

		result
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

	/// Looks up the `(pattern, qos)` registered for a subscription `id`.
	///
	/// Fails with [`TopicRouterError::SubscriptionNotFound`] if `id` is unknown.
	pub fn get_topic_by_id(
		&self,
		id: &SubscriptionId,
	) -> Result<&(TopicPatternPath, QoS), TopicRouterError> {
		self.subscriptions
			.get(id)
			.ok_or(TopicRouterError::subscription_not_found(*id))
	}
}
