use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

use arcstr::ArcStr;

use super::topic_matcher::TopicMatcherNode;
use super::topic_pattern_path::TopicPatternPath;
use crate::routing::subscription_manager::CacheStrategy;
use crate::topic::topic_match::TopicPath;

/// A subscription identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId {
	pub id: usize,
}

// Helper function to create a unique SubscriptionId
fn make_subscription_id(id: usize) -> SubscriptionId {
	SubscriptionId { id }
}

fn new_from_string(pattern: &str) -> Result<TopicPatternPath, String> {
	TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
		.map_err(|e| e.to_string())
}

// Helper function to test subscription matching
fn test_subscriptions(
	// Map of pattern strings to subscription IDs
	subscriptions: &[(&str, usize)],
	// Map of topic paths to expected matching subscription IDs
	expected_matches: &[(&str, Vec<usize>)],
) {
	// Create the matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Subscribe to all patterns
	for (pattern_str, sub_id) in subscriptions {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(*sub_id);
		root.subscribe_to_pattern(&pattern).insert(sub_id);
	}

	// Test all expected matches
	for (path, expected_sub_ids) in expected_matches {
		// Create HashSet of expected subscription IDs
		let expected: HashSet<SubscriptionId> = expected_sub_ids
			.iter()
			.map(|id| make_subscription_id(*id))
			.collect();

		// Get actual matches
		let topic = TopicPath::new(ArcStr::from(*path));
		let matches = root.find_by_path(&topic);

		// Collect all unique subscription IDs from matches
		let actual: HashSet<SubscriptionId> =
			matches.iter().flat_map(|set| set.iter().cloned()).collect();

		// Assert that actual matches equal expected matches
		assert_eq!(
			actual, expected,
			"Path '{}' matched subscriptions {:?}, expected {:?}",
			path, actual, expected
		);
	}
}

#[test]
fn test_exact_matches() {
	// Subscriptions: (pattern, subscription_id)
	let subscriptions = [
		("sensors/temperature", 1),
		("sensors/humidity", 2),
		("devices/light/status", 3),
	];

	// Expected matches: (path, [subscription_ids])
	let expected_matches = [
		("sensors/temperature", vec![1]),
		("sensors/humidity", vec![2]),
		("devices/light/status", vec![3]),
		("sensors/pressure", vec![]), // No matches
	];

	test_subscriptions(&subscriptions, &expected_matches);
}

#[test]
fn test_plus_wildcards() {
	let subscriptions = [
		("sensors/+/reading", 1),
		("devices/+/+/state", 2),
		("home/+", 3),
	];

	let expected_matches = [
		("sensors/temperature/reading", vec![1]),
		("sensors/humidity/reading", vec![1]),
		("sensors/temperature/value", vec![]), // No match
		("devices/light/kitchen/state", vec![2]),
		("devices/light/state", vec![]), // No match (wrong segments)
		("home/kitchen", vec![3]),
		("home/livingroom", vec![3]),
		("home/kitchen/temperature", vec![]), // No match (extra segment)
	];

	test_subscriptions(&subscriptions, &expected_matches);
}

#[test]
fn test_hash_wildcards() {
	let subscriptions = [
		("sensors/#", 1),
		("home/livingroom/#", 2),
		("#", 3), // Match everything
	];

	let expected_matches = [
		("sensors", vec![1, 3]),
		("sensors/temperature", vec![1, 3]),
		("sensors/kitchen/temperature", vec![1, 3]),
		("home/livingroom", vec![2, 3]),
		("home/livingroom/light", vec![2, 3]),
		("home/kitchen", vec![3]), // Only matches #
		                           //("", vec![3]),  // Empty path not allowed
	];

	test_subscriptions(&subscriptions, &expected_matches);
}

#[test]
fn test_complex_subscriptions() {
	let subscriptions = [
		("home/kitchen/temperature", 1), // Exact match
		("home/+/temperature", 2),       // Single-level wildcard
		("home/kitchen/+", 3),           // Another single-level wildcard
		("home/#", 4),                   // Multi-level wildcard
		("+/kitchen/#", 5),              // Mixed wildcards
	];

	let expected_matches = [
		("home/kitchen/temperature", vec![1, 2, 3, 4, 5]), // Matches all patterns
		("home/livingroom/temperature", vec![2, 4]),       // Matches patterns 2, 4
		("home/kitchen/humidity", vec![3, 4, 5]),          // Matches patterns 3, 4, 5
		("home/kitchen/temperature/celsius", vec![4, 5]), // Matches only multi-level wildcards
		("office/kitchen/temperature", vec![5]),          // Matches only pattern 5
		("home", vec![4]),                                // Matches only pattern 4
	];

	test_subscriptions(&subscriptions, &expected_matches);
}

#[test]
fn test_edge_cases() {
	let subscriptions = [
		//("", 1),          // Empty pattern is not valid
		("#", 2),   // Root-level wildcard
		("+", 3),   // Single segment wildcard
		("+/+", 4), // Two wildcards
		("+/#", 5), // Wildcard combination
	];

	let expected_matches = [
		//
		//("", vec![2, 3, 5]),          // Matches #, +, and +/# (not empty pattern)
		("segment", vec![2, 3, 5]), // Matches #, +, and +/#
		("segment1/segment2", vec![2, 4, 5]), // Matches #, +/+, and +/#
		("segment1/segment2/segment3", vec![2, 5]), // Matches # and +/#
	];

	// Note: When using HashSet to store the test results, we only care about set membership,
	// not the specific HashSet implementation details which may cause order differences
	//
	// Also note: Empty path "" doesn't match with empty pattern "". This is because empty path results in
	// path_segments with one empty string element [""], not an empty array []. The current implementation
	// treats this as a single empty segment, not as an exact match for an empty topic pattern.

	test_subscriptions(&subscriptions, &expected_matches);
}

#[test]
fn test_multiple_subscribers_to_same_pattern() {
	// Create matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Multiple subscriptions to the same pattern
	let pattern = new_from_string("sensors/temperature").unwrap();

	let sub_id1 = make_subscription_id(1);
	let sub_id2 = make_subscription_id(2);

	// Add multiple subscription IDs to the same pattern
	let subscribers = root.subscribe_to_pattern(&pattern);
	subscribers.insert(sub_id1.clone());
	subscribers.insert(sub_id2.clone());

	// Test that both subscriptions match
	let topic = TopicPath::new(ArcStr::from("sensors/temperature"));
	let matches = root.find_by_path(&topic);
	assert_eq!(matches.len(), 1); // One matching node

	let matched_subs: HashSet<SubscriptionId> =
		matches[0].iter().cloned().collect();
	let expected_subs: HashSet<SubscriptionId> =
		[sub_id1, sub_id2].into_iter().collect();

	assert_eq!(matched_subs, expected_subs);
}

#[test]
fn test_same_subscriber_multiple_patterns() {
	// Create matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Same subscriber subscribes to multiple patterns
	let sub_id = make_subscription_id(1);

	// Subscribe to pattern 1: Exact match
	let pattern1 = new_from_string("devices/living-room/temperature").unwrap();
	root.subscribe_to_pattern(&pattern1).insert(sub_id.clone());

	// Subscribe to pattern 2: Wildcard match
	let pattern2 = new_from_string("devices/+/humidity").unwrap();
	root.subscribe_to_pattern(&pattern2).insert(sub_id.clone());

	// Subscribe to pattern 3: Hash wildcard
	let pattern3 = new_from_string("sensors/#").unwrap();
	root.subscribe_to_pattern(&pattern3).insert(sub_id.clone());

	// Test each path individually
	{
		let topic =
			TopicPath::new(ArcStr::from("devices/living-room/temperature"));
		let temp_matches = root.find_by_path(&topic);
		let matched_subs = collect_sub_ids(&temp_matches);

		assert_eq!(matched_subs.len(), 1);
		assert!(matched_subs.contains(&sub_id));
	}

	{
		let topic =
			TopicPath::new(ArcStr::from("devices/living-room/humidity"));
		let humidity_matches = root.find_by_path(&topic);
		let matched_subs = collect_sub_ids(&humidity_matches);

		assert_eq!(matched_subs.len(), 1);
		assert!(matched_subs.contains(&sub_id));
	}

	{
		let topic =
			TopicPath::new(ArcStr::from("sensors/living-room/temperature"));
		let sensors_matches = root.find_by_path(&topic);
		let matched_subs = collect_sub_ids(&sensors_matches);

		assert_eq!(matched_subs.len(), 1);
		assert!(matched_subs.contains(&sub_id));
	}
}

// Helper function to collect all subscriber IDs from matches
fn collect_sub_ids(
	matches: &[&HashSet<SubscriptionId>],
) -> HashSet<SubscriptionId> {
	matches.iter().flat_map(|set| set.iter().cloned()).collect()
}

// Helper function to test active subscriptions
fn test_active_subscriptions(
	// Map of pattern strings to subscription IDs (for adding subscriptions)
	subscriptions: &[(&str, usize)],
	// Expected active subscription patterns with their counts: (pattern, count)
	expected_active: &[(String, usize)],
) {
	// Create the matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Subscribe to all patterns
	for (pattern_str, sub_id) in subscriptions {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(*sub_id);
		root.subscribe_to_pattern(&pattern).insert(sub_id);
	}

	// Get actual active subscriptions
	let active = root
		.active_subscriptions()
		.into_iter()
		.map(|(path, d)| (path.to_string(), d.len()));

	// Convert expected to a HashMap for easier comparison
	let mut expected_map: HashMap<String, usize> = HashMap::new();
	for (pattern, count) in expected_active {
		expected_map.insert(pattern.into(), *count);
	}

	// Convert actual to a HashMap for comparison
	let actual_map: HashMap<_, _> = active.into_iter().collect();

	// Assert that actual active subscriptions match expected
	assert_eq!(
		actual_map, expected_map,
		"Active subscriptions {:?}, expected {:?}",
		actual_map, expected_map
	);
}

#[test]
fn test_simple_active_subscriptions() {
	// Subscriptions: (pattern, subscription_id)
	let subscriptions = [
		("sensors/temperature", 1),
		("sensors/humidity", 2),
		("devices/light/status", 3),
	];

	// Expected active subscriptions: (pattern, count)
	let expected_active = [
		("sensors/temperature".to_string(), 1),
		("sensors/humidity".to_string(), 1),
		("devices/light/status".to_string(), 1),
	];

	test_active_subscriptions(&subscriptions, &expected_active);
}

#[test]
fn test_wildcards_active_subscriptions() {
	let subscriptions = [
		("sensors/+/reading", 1),
		("devices/+/+/state", 2),
		("home/#", 3),
		("#", 4),
		("/", 5),
	];

	let expected_active = [
		("sensors/+/reading".to_string(), 1),
		("devices/+/+/state".to_string(), 1),
		("home/#".to_string(), 1),
		("#".to_string(), 1),
		("/".to_string(), 1),
	];

	test_active_subscriptions(&subscriptions, &expected_active);
}

#[test]
fn test_multiple_subs_active_subscriptions() {
	// Create matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Multiple subscriptions to the same pattern
	let pattern1 = new_from_string("sensors/temperature").unwrap();
	let pattern2 = new_from_string("home/#").unwrap();

	// Add multiple subscription IDs to the first pattern
	let subscribers1 = root.subscribe_to_pattern(&pattern1);
	subscribers1.insert(make_subscription_id(1));
	subscribers1.insert(make_subscription_id(2));
	subscribers1.insert(make_subscription_id(3));

	// Add multiple subscription IDs to the second pattern
	let subscribers2 = root.subscribe_to_pattern(&pattern2);
	subscribers2.insert(make_subscription_id(4));
	subscribers2.insert(make_subscription_id(5));

	// Expected active subscriptions
	let expected: HashMap<String, usize> = [
		("sensors/temperature".to_string(), 3),
		("home/#".to_string(), 2),
	]
	.into_iter()
	.collect();

	// Get actual active subscriptions
	let active = root.active_subscriptions();
	let actual: HashMap<String, usize> = active
		.into_iter()
		.map(|(s, d)| (s.to_string(), d.len()))
		.collect();

	assert_eq!(
		actual, expected,
		"Active subscriptions {:?}, expected {:?}",
		actual, expected
	);
}

// Helper function for testing subscription and unsubscription
fn test_update_node(
	// Initial subscriptions to add: (pattern, subscription_id)
	initial_subs: &[(&str, usize)],
	// Operations to perform: (pattern, subscription_id, should_remove)
	operations: &[(&str, usize, bool)],
	// Expected active subscriptions after operations: (pattern, count)
	expected_active: &[(String, usize)],
) {
	// Create the matcher node
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();

	// Add initial subscriptions
	for (pattern_str, sub_id) in initial_subs {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(*sub_id);
		root.subscribe_to_pattern(&pattern).insert(sub_id);
	}

	// Perform operations (subscribe or unsubscribe)
	for (pattern_str, sub_id, should_remove) in operations {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(*sub_id);

		// Manually update the subscription in the node

		if *should_remove {
			root.update_node(pattern.slice(), |subscriptions| {
				subscriptions.remove(&sub_id);
			})
			.unwrap();
		} else {
			let subscriptions = root.subscribe_to_pattern(&pattern);
			subscriptions.insert(sub_id);
		}
	}

	// Check the resulting active subscriptions
	let active = root.active_subscriptions();

	// Convert expected to a HashMap for easier comparison
	let mut expected_map: HashMap<String, usize> = HashMap::new();
	for (pattern, count) in expected_active {
		expected_map.insert(pattern.clone(), *count);
	}

	// Convert actual to a HashMap for comparison
	let actual_map: HashMap<String, usize> = active
		.into_iter()
		.map(|(s, d)| (s.to_string(), d.len()))
		.collect();

	// Assert that actual active subscriptions match expected
	assert_eq!(
		actual_map, expected_map,
		"After update operations, active subscriptions are {:?}, expected {:?}",
		actual_map, expected_map
	);
}

#[test]
fn test_simple_unsubscribe() {
	// Initial subscriptions
	let initial_subs = [
		("sensors/temperature", 1),
		("sensors/humidity", 2),
		("devices/light/status", 3),
	];

	// Unsubscribe operations
	let operations = [
		("sensors/temperature", 1, true), // Unsubscribe ID 1 from sensors/temperature
	];

	// Expected result after operations
	let expected_active = [
		("sensors/humidity".to_string(), 1),
		("devices/light/status".to_string(), 1),
	];

	test_update_node(&initial_subs, &operations, &expected_active);
}

#[test]
fn test_wildcard_unsubscribe() {
	// Initial subscriptions
	let initial_subs = [
		("sensors/+/reading", 1),
		("devices/+/+/state", 2),
		("home/#", 3),
	];

	// Unsubscribe operations
	let operations = [
		("home/#", 3, true), // Unsubscribe ID 3 from home/#
		("devices/+/+/state", 2, true), // Unsubscribe ID 2 from devices/+/+/state
	];

	// Expected result after operations
	let expected_active = [("sensors/+/reading".to_string(), 1)];

	test_update_node(&initial_subs, &operations, &expected_active);
}

#[test]
fn test_multiple_subscribers_unsubscribe() {
	// Initial subscriptions
	let initial_subs = [
		("sensors/temperature", 1),
		("sensors/temperature", 2),
		("sensors/temperature", 3),
		("home/#", 4),
		("home/#", 5),
	];

	// Unsubscribe operations
	let operations = [
		("sensors/temperature", 2, true), // Unsubscribe ID 2 from sensors/temperature
		("home/#", 4, true),              // Unsubscribe ID 4 from home/#
	];

	// Expected result after operations
	let expected_active = [
		("sensors/temperature".to_string(), 2), // IDs 1 and 3 still subscribed
		("home/#".to_string(), 1),              // ID 5 still subscribed
	];

	test_update_node(&initial_subs, &operations, &expected_active);
}

#[test]
fn test_mixed_operations() {
	// Initial subscriptions
	let initial_subs = [("sensors/temperature", 1), ("home/+/light", 2)];

	// Mixed operations: subscribe and unsubscribe
	let operations = [
		("sensors/temperature", 1, true), // Unsubscribe ID 1
		("sensors/humidity", 3, false),   // Subscribe ID 3
		("home/+/light", 4, false),       // Subscribe ID 4
		("devices/#", 5, false),          // Subscribe ID 5
	];

	// Expected result after operations
	let expected_active = [
		("sensors/humidity".to_string(), 1),
		("home/+/light".to_string(), 2), // ID 2 and 4
		("devices/#".to_string(), 1),
	];

	test_update_node(&initial_subs, &operations, &expected_active);
}

#[test]
fn test_unsubscribe_all() {
	// Initial subscriptions
	let initial_subs = [
		("sensors/temperature", 1),
		("sensors/humidity", 2),
		("home/#", 3),
	];

	// Unsubscribe all
	let operations = [
		("sensors/temperature", 1, true),
		("sensors/humidity", 2, true),
		("home/#", 3, true),
	];

	// Expected result: empty
	let expected_active: [(String, usize); 0] = [];

	test_update_node(&initial_subs, &operations, &expected_active);

	// Verify with a new matcher that it's truly empty
	let mut root = TopicMatcherNode::<HashSet<SubscriptionId>>::new();
	for (pattern_str, sub_id) in initial_subs {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(sub_id);
		root.subscribe_to_pattern(&pattern).insert(sub_id);
	}

	for (pattern_str, sub_id, _) in operations {
		let pattern = new_from_string(pattern_str).unwrap();
		let sub_id = make_subscription_id(sub_id);
		root.update_node(pattern.slice(), |subscriptions| {
			subscriptions.remove(&sub_id);
		})
		.unwrap();
	}

	assert!(
		root.is_empty(),
		"Root node should be empty after unsubscribing all:{:#?}",
		root
	);
}
