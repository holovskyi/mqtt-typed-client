//! Cache strategy configuration for topic pattern matching
//!
//! Provides configuration options for caching topic matching results
//! to improve performance for frequently used patterns.

use std::num::NonZeroUsize;

/// Strategy for caching topic matching results
///
/// Controls how topic match results are cached to optimize performance.
/// Different strategies trade memory usage for lookup speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStrategy {
	/// Use LRU (Least Recently Used) cache with a fixed size
	///
	/// Maintains a cache of recently matched topics. When the cache is full,
	/// the least recently used entry is evicted. Provides good performance
	/// for workloads with repeated topic patterns.
	///
	/// **Note:** This variant is only available with the `lru-cache` feature enabled.
	///
	/// # Example
	/// ```ignore
	/// use std::num::NonZeroUsize;
	/// use mqtt_topic_engine::CacheStrategy;
	///
	/// let cache = CacheStrategy::Lru(NonZeroUsize::new(100).unwrap());
	/// ```
	#[cfg(feature = "lru-cache")]
	Lru(NonZeroUsize),

	/// No caching - always create new TopicPath instances
	///
	/// Disables caching entirely. Use this when:
	/// - Topic patterns are rarely repeated
	/// - Memory is constrained
	/// - Simplicity is preferred over performance
	NoCache,
}

impl CacheStrategy {
	/// Create a new cache strategy with the specified capacity
	///
	/// Returns `NoCache` if capacity is 0, otherwise returns `Lru` with the given capacity
	/// (if `lru-cache` feature is enabled).
	///
	/// # Arguments
	/// * `capacity` - Maximum number of entries to cache (0 means no caching)
	///
	/// # Examples
	/// ```
	/// use mqtt_topic_engine::CacheStrategy;
	///
	/// // Create no-cache strategy
	/// let no_cache = CacheStrategy::new(0);
	/// assert_eq!(no_cache, CacheStrategy::NoCache);
	/// ```
	///
	/// With `lru-cache` feature:
	/// ```ignore
	/// // Create LRU cache with 100 entries
	/// let cache = CacheStrategy::new(100);
	/// ```
	pub fn new(capacity: usize) -> Self {
		if capacity == 0 {
			Self::NoCache
		} else {
			#[cfg(feature = "lru-cache")]
			{
				Self::Lru(
					NonZeroUsize::new(capacity).expect("Capacity must be > 0"),
				)
			}
			#[cfg(not(feature = "lru-cache"))]
			{
				tracing::warn!(
					capacity,
					"LRU cache requested with capacity {}, but 'lru-cache' \
					 feature is disabled. Falling back to NoCache. Enable \
					 'lru-cache' feature in Cargo.toml to use caching.",
					capacity
				);
				Self::NoCache
			}
		}
	}

	/// Returns the cache capacity if using LRU strategy
	///
	/// # Examples
	/// ```
	/// use mqtt_topic_engine::CacheStrategy;
	///
	/// let no_cache = CacheStrategy::NoCache;
	/// assert_eq!(no_cache.capacity(), None);
	/// ```
	///
	/// With `lru-cache` feature:
	/// ```ignore
	/// use std::num::NonZeroUsize;
	/// use mqtt_topic_engine::CacheStrategy;
	///
	/// let cache = CacheStrategy::new(100);
	/// assert_eq!(cache.capacity(), Some(NonZeroUsize::new(100).unwrap()));
	/// ```
	pub fn capacity(&self) -> Option<NonZeroUsize> {
		match self {
			#[cfg(feature = "lru-cache")]
			| Self::Lru(size) => Some(*size),
			| Self::NoCache => None,
		}
	}
}

impl Default for CacheStrategy {
	/// Default strategy is no caching
	///
	/// This ensures predictable behavior and minimal memory usage by default.
	fn default() -> Self {
		Self::NoCache
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[cfg(feature = "lru-cache")]
	fn test_new_with_capacity() {
		let cache = CacheStrategy::new(100);
		assert!(matches!(cache, CacheStrategy::Lru(_)));
		assert_eq!(cache.capacity(), NonZeroUsize::new(100));
	}

	#[test]
	#[cfg(not(feature = "lru-cache"))]
	fn test_new_with_capacity_no_lru() {
		// Without lru-cache feature, new(100) should return NoCache
		let cache = CacheStrategy::new(100);
		assert_eq!(cache, CacheStrategy::NoCache);
		assert_eq!(cache.capacity(), None);
	}

	#[test]
	fn test_new_with_zero_capacity() {
		let cache = CacheStrategy::new(0);
		assert_eq!(cache, CacheStrategy::NoCache);
		assert_eq!(cache.capacity(), None);
	}

	#[test]
	fn test_default() {
		let cache = CacheStrategy::default();
		assert_eq!(cache, CacheStrategy::NoCache);
	}

	#[test]
	#[cfg(feature = "lru-cache")]
	fn test_capacity() {
		let lru = CacheStrategy::Lru(NonZeroUsize::new(50).unwrap());
		assert_eq!(lru.capacity(), NonZeroUsize::new(50));

		let no_cache = CacheStrategy::NoCache;
		assert_eq!(no_cache.capacity(), None);
	}

	#[test]
	fn test_capacity_no_cache() {
		let no_cache = CacheStrategy::NoCache;
		assert_eq!(no_cache.capacity(), None);
	}

	#[test]
	fn test_clone_and_copy() {
		let cache = CacheStrategy::new(100);
		let cloned = cache;
		assert_eq!(cache, cloned);
	}
}
