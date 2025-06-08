# Topic Module Error System Documentation

## Overview

This document describes the comprehensive error handling system implemented for the MQTT typed client's topic module. The system follows Rust best practices using `thiserror` for structured error handling while maintaining type safety and clear error propagation.

## What Was Implemented

### 1. Error Type Hierarchy

A modular error system was created with three specialized error types and one composite type:

#### `TopicPatternError` (Modernized)
- **Purpose**: Handles topic pattern parsing and validation errors
- **Changes**: Migrated from manual `Display`/`Error` implementation to `thiserror`
- **Variants**:
  - `HashPosition { pattern: String }` - Hash wildcard (#) not at pattern end
  - `WildcardUsage { usage: String }` - Invalid wildcard character usage
  - `EmptyTopic` - Empty topic pattern provided

#### `TopicMatcherError` (New)
- **Purpose**: Handles topic matching operation errors
- **Variants**:
  - `EmptyTopicPath` - Empty topic path for matching
  - `InvalidSegment { segment: String, position: usize }` - Invalid topic segment
  - `InvalidUtf8 { details: String }` - UTF-8 encoding issues

#### `TopicRouterError` (New)
- **Purpose**: Handles topic routing operation errors
- **Variants**:
  - `InvalidPattern(TopicPatternError)` - Pattern validation failed (auto-conversion)
  - `MatchingFailed(TopicMatcherError)` - Matching operation failed (auto-conversion)
  - `SubscriptionNotFound { id: SubscriptionId }` - Subscription ID not found
  - `InvalidRoutingTopic { topic: String, reason: String }` - Invalid topic for routing
  - `InternalStateCorrupted { details: String }` - Internal state corruption

#### `TopicError` (Composite)
- **Purpose**: Unified error type for the entire topic module
- **Variants**:
  - `Pattern(TopicPatternError)` - Pattern-related errors (auto-conversion)
  - `Matcher(TopicMatcherError)` - Matcher-related errors (auto-conversion)
  - `Router(TopicRouterError)` - Router-related errors (auto-conversion)

### 2. Convenient Result Types

Type aliases were created for better ergonomics:

```rust
pub type TopicResult<T> = Result<T, TopicError>;
pub type PatternResult<T> = Result<T, TopicPatternError>;
pub type MatcherResult<T> = Result<T, TopicMatcherError>;
pub type RouterResult<T> = Result<T, TopicRouterError>;
```

### 3. Error Constructor Methods

Convenient constructor methods were added to all error types:

```rust
// TopicPatternError constructors
TopicPatternError::hash_position(\"invalid/#/pattern\")
TopicPatternError::wildcard_usage(\"invalid++\")

// TopicMatcherError constructors
TopicMatcherError::invalid_segment(\"bad-segment\", 2)
TopicMatcherError::invalid_utf8(\"encoding details\")

// TopicRouterError constructors
TopicRouterError::subscription_not_found(id)
TopicRouterError::invalid_routing_topic(\"topic\", \"reason\")
TopicRouterError::internal_state_corrupted(\"details\")
```

### 4. Error Classification Methods

Methods for error categorization and handling decisions:

```rust
// Behavioral classification
error.is_client_error()     // Client-side vs server-side error
error.is_retryable()        // Can retry operation
error.is_recoverable()      // Can recover from error
error.is_validation_error() // Input validation error

// Categorization for logging/monitoring
error.category()     // \"pattern\", \"matcher\", \"router\"
error.error_type()   // Specific error type string
```

### 5. Automatic Error Conversions

Seamless error propagation using `#[from]` attributes:

```rust
// These conversions work automatically with the ? operator
TopicPatternError -> TopicRouterError -> TopicError
TopicMatcherError -> TopicRouterError -> TopicError
TopicPatternError -> TopicError
TopicMatcherError -> TopicError
```

### 6. Updated Module Exports

All error types are properly exported from the topic module:

```rust
pub use topic_matcher::{IsEmpty, Len, TopicMatcherError, TopicMatcherNode};
pub use topic_pattern_path::{TopicPatternError, TopicPatternItem, TopicPatternPath};
pub use topic_router::{SubscriptionId, TopicRouter, TopicRouterError};
```

## Dependencies Added

- `thiserror = \"2.0\"` - For structured error handling

## Testing

- ✅ All existing tests pass (31/31)
- ✅ Code compiles without warnings
- ✅ Backward compatibility maintained
- ✅ Updated test cases for new error variants

## What Needs to Be Done Next

To complete the error handling implementation in the topic module, the following steps are required:

### Phase 1: Replace Panic-Prone Code

#### 1.1 Topic Matcher (`topic_matcher.rs`)
- **Current Issues**: Uses `unwrap()` in several places
- **Action Required**: 
  - Add validation to `find_by_path()` method
  - Return `MatcherResult<Vec<&T>>` instead of `Vec<&T>`
  - Validate topic path segments for UTF-8 and emptiness
  - Handle invalid segment scenarios gracefully

```rust
// Example transformation needed:
// Before:
pub fn find_by_path<'a>(&'a self, path: &str) -> Vec<&'a T> {
    let path_segments: Vec<&str> = path.split('/').collect();
    // ...
}

// After:
pub fn find_by_path<'a>(&'a self, path: &str) -> MatcherResult<Vec<&'a T>> {
    if path.is_empty() {
        return Err(TopicMatcherError::EmptyTopicPath);
    }
    
    if !path.is_ascii() {
        return Err(TopicMatcherError::invalid_utf8(\"Non-ASCII characters in path\"));
    }
    
    let path_segments: Vec<&str> = path.split('/').collect();
    // ... rest of implementation
    Ok(matching_subscribers)
}
```

#### 1.2 Topic Router (`topic_router.rs`)
- **Current Issues**: Methods don't return errors, use panics implicitly
- **Action Required**:
  - Update `subscribe()` method to return `RouterResult<(bool, SubscriptionId)>`
  - Update `unsubscribe()` method to return `RouterResult<Option<(bool, TopicPatternPath)>>`
  - Update `get_subscribers()` method to return `RouterResult<Vec<(&SubscriptionId, &T)>>`
  - Add validation for topic routing operations

```rust
// Example transformation needed:
// Before:
pub fn subscribe(&mut self, topic: TopicPatternPath, subscription: T) -> (bool, SubscriptionId) {
    // ...
}

// After:
pub fn subscribe(&mut self, topic: TopicPatternPath, subscription: T) -> RouterResult<(bool, SubscriptionId)> {
    // Validate topic pattern is reasonable for subscription
    if topic.len() > MAX_TOPIC_DEPTH {
        return Err(TopicRouterError::invalid_routing_topic(
            topic.to_string(),
            \"Topic depth exceeds maximum allowed\"
        ));
    }
    
    let id = SubscriptionId(self.next_id);
    self.next_id = self.next_id.wrapping_add(1);
    
    let subscription_table = self.topic_matcher.subscribe_to_pattern(&topic)?;
    let is_empty = subscription_table.is_empty();
    subscription_table.insert(id, subscription);
    self.subscriptions.insert(id, topic);
    
    Ok((is_empty, id))
}
```

#### 1.3 Topic Pattern Path (`topic_pattern_path.rs`)
- **Current Status**: ✅ Already properly implemented with error handling
- **No Action Required**: This module already uses proper error handling

### Phase 2: Add Input Validation

#### 2.1 Topic Depth Limits
```rust
const MAX_TOPIC_DEPTH: usize = 32;
const MAX_SEGMENT_LENGTH: usize = 256;
```

#### 2.2 Topic Content Validation
- Validate topic segments don't contain null bytes
- Validate reasonable topic length limits
- Validate wildcard usage rules

#### 2.3 State Consistency Checks
- Add internal state validation methods
- Detect and report corruption scenarios
- Add recovery mechanisms where possible

### Phase 3: Update Method Signatures

#### 3.1 Public API Methods
Update all public methods to return appropriate Result types:

```rust
// TopicRouter public API
impl<T> TopicRouter<T> {
    pub fn subscribe(&mut self, topic: TopicPatternPath, subscription: T) -> RouterResult<(bool, SubscriptionId)>
    pub fn unsubscribe(&mut self, id: &SubscriptionId) -> RouterResult<Option<(bool, TopicPatternPath)>>
    pub fn get_subscribers<'a>(&'a self, topic: &Topic) -> RouterResult<Vec<(&'a SubscriptionId, &'a T)>>
    pub fn get_active_subscriptions(&self) -> RouterResult<Vec<TopicPatternPath>>
}

// TopicMatcherNode public API  
impl<T: Default + IsEmpty + Len> TopicMatcherNode<T> {
    pub fn find_by_path<'a>(&'a self, path: &str) -> MatcherResult<Vec<&'a T>>
    pub fn subscribe_to_pattern(&mut self, topic_path: &TopicPatternPath) -> MatcherResult<&mut T>
    pub fn update_node<F>(&mut self, topic_path: &[TopicPatternItem], f: F) -> MatcherResult<bool>
        where F: FnMut(&mut T)
}
```

### Phase 4: Error Integration Testing

#### 4.1 Create Error Scenario Tests
```rust
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_empty_topic_path_error() {
        let matcher = TopicMatcherNode::<HashSet<SubscriptionId>>::new();
        let result = matcher.find_by_path(\"\");
        assert!(matches!(result, Err(TopicMatcherError::EmptyTopicPath)));
    }

    #[test] 
    fn test_invalid_utf8_topic() {
        // Test with invalid UTF-8 sequences
    }

    #[test]
    fn test_subscription_not_found() {
        let mut router = TopicRouter::new();
        let result = router.unsubscribe(&SubscriptionId(999));
        assert!(matches!(result, Err(TopicRouterError::SubscriptionNotFound { .. })));
    }

    #[test]
    fn test_topic_depth_limit() {
        // Test with extremely deep topics
    }
}
```

#### 4.2 Integration Tests
- Test error propagation through the entire call stack
- Test error conversion chains
- Test error recovery scenarios

### Phase 5: Documentation and Examples

#### 5.1 Update Documentation
- Add error handling examples to module documentation
- Document error scenarios for each public method
- Add troubleshooting guide for common errors

#### 5.2 Usage Examples
```rust
// Example of proper error handling
use mqtt_typed_client::topic::*;

fn handle_subscription(router: &mut TopicRouter<String>, pattern: &str) -> TopicResult<SubscriptionId> {
    let topic_pattern = TopicPatternPath::try_from(pattern)?;
    let (_, sub_id) = router.subscribe(topic_pattern, \"subscription_data\".to_string())?;
    Ok(sub_id)
}

fn handle_topic_matching(router: &TopicRouter<String>, topic: &str) -> TopicResult<Vec<String>> {
    let subscribers = router.get_subscribers(&topic.into())?;
    Ok(subscribers.into_iter().map(|(_, data)| data.clone()).collect())
}
```

## Benefits of This Implementation

1. **Type Safety**: Impossible to ignore errors at compile time
2. **Clear Error Propagation**: `?` operator works seamlessly across all layers
3. **Structured Error Information**: Rich error context for debugging and logging
4. **Consistent API**: All error types follow the same patterns
5. **Future-Proof**: Easy to add new error variants without breaking changes
6. **Performance**: Zero-cost abstractions with compile-time optimizations
7. **Maintainable**: Clear separation of concerns between error types

## Migration Strategy

The implementation can be done incrementally:

1. **Phase 1** can be implemented gradually, method by method
2. **Existing code continues to work** during migration
3. **Tests provide safety net** for refactoring
4. **Error types are already in place** and ready to use

This approach ensures the module becomes more robust and maintainable while providing excellent developer experience through clear error messages and type-safe error handling.
