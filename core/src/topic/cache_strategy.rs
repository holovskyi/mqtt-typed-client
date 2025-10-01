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
	/// # Example
	/// ```
	/// use std::num::NonZeroUsize;
	/// use mqtt_typed_client_core::topic::CacheStrategy;
	///
	/// let cache = CacheStrategy::Lru(NonZeroUsize::new(100).unwrap());
	/// ```
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
	/// Returns `NoCache` if capacity is 0, otherwise returns `Lru` with the given capacity.
	///
	/// # Arguments
	/// * `capacity` - Maximum number of entries to cache (0 means no caching)
	///
	/// # Examples
	/// ```
	/// use mqtt_typed_client_core::topic::CacheStrategy;
	///
	/// // Create LRU cache with 100 entries
	/// let cache = CacheStrategy::new(100);
	///
	/// // Create no-cache strategy
	/// let no_cache = CacheStrategy::new(0);
	/// assert_eq!(no_cache, CacheStrategy::NoCache);
	/// ```
	pub fn new(capacity: usize) -> Self {
		if capacity == 0 {
			Self::NoCache
		} else {
			Self::Lru(
				NonZeroUsize::new(capacity).expect("Capacity must be > 0"),
			)
		}
	}

	/// Returns the cache capacity if using LRU strategy
	///
	/// # Examples
	/// ```
	/// use std::num::NonZeroUsize;
	/// use mqtt_typed_client_core::topic::CacheStrategy;
	///
	/// let cache = CacheStrategy::new(100);
	/// assert_eq!(cache.capacity(), Some(NonZeroUsize::new(100).unwrap()));
	///
	/// let no_cache = CacheStrategy::NoCache;
	/// assert_eq!(no_cache.capacity(), None);
	/// ```
	pub fn capacity(&self) -> Option<NonZeroUsize> {
		match self {
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
	fn test_new_with_capacity() {
		let cache = CacheStrategy::new(100);
		assert!(matches!(cache, CacheStrategy::Lru(_)));
		assert_eq!(cache.capacity(), NonZeroUsize::new(100));
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
	fn test_capacity() {
		let lru = CacheStrategy::Lru(NonZeroUsize::new(50).unwrap());
		assert_eq!(lru.capacity(), NonZeroUsize::new(50));

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
