//! Tests for TopicPatternItem functionality

use arcstr::Substr;

use super::{TopicPatternError, TopicPatternItem};

#[test]
fn test_literal_string_item() {
	let item = TopicPatternItem::try_from(Substr::from("sensors")).unwrap();

	assert_eq!(item, TopicPatternItem::Str(Substr::from("sensors")));
	assert_eq!(item.as_str(), "sensors");
	assert_eq!(item.as_wildcard(), "sensors");
	assert_eq!(item.param_name(), None);
	assert!(!item.is_wildcard());
}

#[test]
fn test_anonymous_plus_wildcard() {
	let item = TopicPatternItem::try_from(Substr::from("+")).unwrap();

	assert_eq!(item, TopicPatternItem::Plus(None));
	assert_eq!(item.as_str(), "+");
	assert_eq!(item.as_wildcard(), "+");
	assert_eq!(item.param_name(), None);
	assert!(item.is_wildcard());
}

#[test]
fn test_anonymous_hash_wildcard() {
	let item = TopicPatternItem::try_from(Substr::from("#")).unwrap();

	assert_eq!(item, TopicPatternItem::Hash(None));
	assert_eq!(item.as_str(), "#");
	assert_eq!(item.as_wildcard(), "#");
	assert_eq!(item.param_name(), None);
	assert!(item.is_wildcard());
}

#[test]
fn test_named_plus_wildcard() {
	let item = TopicPatternItem::try_from(Substr::from("{sensor_id}")).unwrap();

	if let TopicPatternItem::Plus(Some(name)) = &item {
		assert_eq!(name.as_str(), "sensor_id");
	} else {
		panic!("Expected Plus with Some name");
	}

	assert_eq!(item.as_str(), "+");
	assert_eq!(item.as_wildcard(), "{sensor_id}");
	assert_eq!(item.param_name().unwrap().as_str(), "sensor_id");
	assert!(item.is_wildcard());
}

#[test]
fn test_named_hash_wildcard() {
	let item = TopicPatternItem::try_from(Substr::from("{details:#}")).unwrap();

	if let TopicPatternItem::Hash(Some(name)) = &item {
		assert_eq!(name.as_str(), "details");
	} else {
		panic!("Expected Hash with Some name");
	}

	assert_eq!(item.as_str(), "#");
	assert_eq!(item.as_wildcard(), "{details:#}");
	assert_eq!(item.param_name().unwrap().as_str(), "details");
	assert!(item.is_wildcard());
}

#[test]
fn test_empty_named_wildcard_error() {
	let result = TopicPatternItem::try_from(Substr::from("{}"));
	assert!(result.is_err());

	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "{}");
	} else {
		panic!("Expected WildcardUsage error");
	}
}

#[test]
fn test_empty_named_hash_wildcard_error() {
	let result = TopicPatternItem::try_from(Substr::from("{:#}"));
	assert!(result.is_err());

	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "{:#}");
	} else {
		panic!("Expected WildcardUsage error");
	}
}

#[test]
fn test_invalid_wildcard_with_text() {
	let result = TopicPatternItem::try_from(Substr::from("text+more"));
	assert!(result.is_err());

	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "text+more");
	} else {
		panic!("Expected WildcardUsage error");
	}
}

#[test]
fn test_invalid_hash_wildcard_with_text() {
	let result = TopicPatternItem::try_from(Substr::from("text#more"));
	assert!(result.is_err());

	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "text#more");
	} else {
		panic!("Expected WildcardUsage error");
	}
}

#[test]
fn test_multiple_plus_wildcards() {
	let result = TopicPatternItem::try_from(Substr::from("++"));
	assert!(result.is_err());

	assert!(matches!(
		result.unwrap_err(),
		TopicPatternError::WildcardUsage { .. }
	));
}

#[test]
fn test_multiple_hash_wildcards() {
	let result = TopicPatternItem::try_from(Substr::from("##"));
	assert!(result.is_err());

	assert!(matches!(
		result.unwrap_err(),
		TopicPatternError::WildcardUsage { .. }
	));
}

#[test]
fn test_display_implementation() {
	let str_item = TopicPatternItem::Str(Substr::from("sensors"));
	let plus_item = TopicPatternItem::Plus(None);
	let hash_item = TopicPatternItem::Hash(None);
	let named_plus = TopicPatternItem::Plus(Some(Substr::from("id")));

	assert_eq!(format!("{str_item}"), "sensors");
	assert_eq!(format!("{plus_item}"), "+");
	assert_eq!(format!("{hash_item}"), "#");
	assert_eq!(format!("{named_plus}"), "+");
}

#[test]
fn test_string_conversion() {
	let item = TopicPatternItem::Str(Substr::from("test"));
	let string_from_item: String = (&item).into();
	assert_eq!(string_from_item, "test");
}

#[test]
fn test_unicode_support() {
	let item = TopicPatternItem::try_from(Substr::from("сенсори")).unwrap();
	assert_eq!(item.as_str(), "сенсори");
	assert!(!item.is_wildcard());
}

#[test]
fn test_special_characters() {
	let item =
		TopicPatternItem::try_from(Substr::from("device-123@home.local"))
			.unwrap();
	assert_eq!(item.as_str(), "device-123@home.local");
	assert!(!item.is_wildcard());
}

#[test]
fn test_empty_string_segment() {
	let item = TopicPatternItem::try_from(Substr::from("")).unwrap();
	assert_eq!(item, TopicPatternItem::Str(Substr::from("")));
	assert_eq!(item.as_str(), "");
	assert!(!item.is_wildcard());
}

#[test]
fn test_named_wildcard_with_underscores() {
	let item =
		TopicPatternItem::try_from(Substr::from("{sensor_id_123}")).unwrap();

	if let TopicPatternItem::Plus(Some(name)) = &item {
		assert_eq!(name.as_str(), "sensor_id_123");
	} else {
		panic!("Expected Plus with Some name");
	}

	assert_eq!(item.as_wildcard(), "{sensor_id_123}");
}

#[test]
fn test_named_wildcard_with_numbers() {
	let item = TopicPatternItem::try_from(Substr::from("{param123}")).unwrap();

	if let TopicPatternItem::Plus(Some(name)) = &item {
		assert_eq!(name.as_str(), "param123");
	} else {
		panic!("Expected Plus with Some name");
	}

	assert_eq!(item.as_wildcard(), "{param123}");
}

#[test]
fn test_malformed_brackets() {
	// Missing closing bracket
	let result1 = TopicPatternItem::try_from(Substr::from("{param"));
	assert!(result1.is_ok()); // This becomes a literal string, not a wildcard

	// Missing opening bracket
	let result2 = TopicPatternItem::try_from(Substr::from("param}"));
	assert!(result2.is_ok()); // This becomes a literal string, not a wildcard

	// Extra brackets
	let result3 = TopicPatternItem::try_from(Substr::from("{{param}}"));
	assert!(result3.is_ok()); // This becomes a literal string, not a wildcard
}

#[test]
fn test_clone_and_equality() {
	let item1 = TopicPatternItem::Plus(Some(Substr::from("test")));
	let item2 = item1.clone();

	assert_eq!(item1, item2);
	assert_eq!(item1.param_name(), item2.param_name());
}

#[test]
fn test_hash_and_debug() {
	use std::collections::HashSet;

	let item1 = TopicPatternItem::Str(Substr::from("test"));
	let item2 = TopicPatternItem::Str(Substr::from("test"));
	let item3 = TopicPatternItem::Plus(None);

	let mut set = HashSet::new();
	set.insert(item1.clone());
	set.insert(item2.clone());
	set.insert(item3.clone());

	// Should only have 2 items since item1 and item2 are equal
	assert_eq!(set.len(), 2);

	// Test Debug implementation
	let debug_str = format!("{item1:?}");
	assert!(debug_str.contains("Str"));
}

#[test]
fn test_invalid_wildcards_in_segments() {
	// Test ++ pattern
	let result = TopicPatternItem::try_from(Substr::from("++"));
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		TopicPatternError::WildcardUsage { .. }
	));

	// Test ## pattern
	let result = TopicPatternItem::try_from(Substr::from("##"));
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		TopicPatternError::WildcardUsage { .. }
	));
}

#[test]
fn test_wildcards_mixed_with_text() {
	// Test + mixed with text
	let result = TopicPatternItem::try_from(Substr::from("a+b"));
	assert!(result.is_err());
	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "a+b");
	} else {
		panic!("Expected WildcardUsage error");
	}

	// Test # mixed with text
	let result = TopicPatternItem::try_from(Substr::from("a#b"));
	assert!(result.is_err());
	if let Err(TopicPatternError::WildcardUsage { usage }) = result {
		assert_eq!(usage, "a#b");
	} else {
		panic!("Expected WildcardUsage error");
	}
}


