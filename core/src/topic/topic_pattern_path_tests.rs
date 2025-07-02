//! Tests for TopicPatternPath functionality

use std::collections::HashMap;

use arcstr::ArcStr;

use super::{TopicPatternError, TopicPatternPath};
use crate::routing::subscription_manager::CacheStrategy;

fn create_pattern(pattern: &str) -> TopicPatternPath {
	TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
		.expect("Pattern should be valid")
}

trait TopicPatternPathTestExt {
	fn with_parameters<I, K, V>(
		self,
		params: I,
	) -> Result<Self, TopicPatternError>
	where
		Self: Sized,
		I: IntoIterator<Item = (K, V)>,
		K: Into<ArcStr>,
		V: Into<ArcStr>;
}

impl TopicPatternPathTestExt for TopicPatternPath {
	fn with_parameters<I, K, V>(
		mut self,
		params: I,
	) -> Result<Self, TopicPatternError>
	where
		I: IntoIterator<Item = (K, V)>,
		K: Into<ArcStr>,
		V: Into<ArcStr>,
	{
		for (key, value) in params {
			self = self.bind_parameter(key, value)?;
		}
		Ok(self)
	}
}

/// Test with_parameters() functionality
mod with_parameters_tests {
	use arcstr::Substr;

	use super::*;
	use crate::topic::TopicPatternItem;

	#[test]
	fn test_single_parameter_substitution() {
		let pattern = create_pattern("sensors/{sensor_id}/data");
		let result = pattern.with_parameters([("sensor_id", "123")]).unwrap();

		// topic_pattern() preserves original template
		assert_eq!(result.topic_pattern(), "sensors/{sensor_id}/data");
		// mqtt_pattern() shows substituted result
		assert_eq!(result.mqtt_pattern(), "sensors/123/data");
	}

	#[test]
	fn test_multiple_parameter_substitution() {
		let pattern =
			create_pattern("buildings/{building}/floors/{floor}/rooms/{room}");
		let params = [("building", "A"), ("floor", "2"), ("room", "kitchen")];
		let result = pattern.with_parameters(params).unwrap();

		// All parameters substituted in mqtt_pattern
		assert_eq!(result.mqtt_pattern(), "buildings/A/floors/2/rooms/kitchen");
	}

	#[test]
	fn test_partial_parameter_substitution() {
		let pattern =
			create_pattern("sensors/{sensor_id}/data/{measurement_type}");
		let result = pattern
			.with_parameters([("sensor_id", "floor_sensor")])
			.unwrap();

		// topic_pattern() preserves original template
		assert_eq!(
			result.topic_pattern(),
			"sensors/{sensor_id}/data/{measurement_type}"
		);
		// mqtt_pattern() shows partial substitution (unbound params become +)
		assert_eq!(result.mqtt_pattern(), "sensors/floor_sensor/data/+");
	}

	#[test]
	fn test_with_hashmap() {
		let pattern =
			create_pattern("devices/{device_type}/{device_id}/status");
		let mut params = HashMap::new();
		params.insert("device_type", "sensor");
		params.insert("device_id", "temp_01");

		let result = pattern.with_parameters(params).unwrap();
		assert_eq!(result.mqtt_pattern(), "devices/sensor/temp_01/status");
	}

	#[test]
	fn test_with_vector_of_tuples() {
		let pattern = create_pattern("home/{room}/device/{device}");
		let params = vec![("room", "kitchen"), ("device", "thermometer")];

		let result = pattern.with_parameters(params).unwrap();
		assert_eq!(result.mqtt_pattern(), "home/kitchen/device/thermometer");
	}

	#[test]
	fn test_with_slice() {
		let pattern = create_pattern("mqtt/{topic}/{subtopic}");
		let params = [("topic", "sensors"), ("subtopic", "temperature")];

		let result = pattern.with_parameters(params).unwrap();
		assert_eq!(result.mqtt_pattern(), "mqtt/sensors/temperature");
	}

	#[test]
	fn test_empty_parameters_returns_clone() {
		let pattern = create_pattern("sensors/{sensor_id}/data");
		let empty_params: Vec<(String, String)> = vec![];
		let topic_pattern = pattern.topic_pattern();
		let mqtt_pattern = pattern.mqtt_pattern();
		let result = pattern.with_parameters(empty_params).unwrap();
		assert_eq!(result.topic_pattern(), topic_pattern);
		assert_eq!(result.mqtt_pattern(), mqtt_pattern);
	}

	#[test]
	fn test_empty_iterator_returns_clone() {
		let pattern = create_pattern("devices/{id}/status");
		let topic_pattern = pattern.topic_pattern();
		let result = pattern
			.with_parameters(std::iter::empty::<(String, String)>())
			.unwrap();

		assert_eq!(result.topic_pattern(), topic_pattern);
	}

	#[test]
	fn test_empty_hashmap_returns_clone() {
		let pattern = create_pattern("sensors/{sensor_id}/data");
		let empty_map: HashMap<String, String> = HashMap::new();
		let topic_pattern = pattern.topic_pattern();
		let result = pattern.with_parameters(&empty_map).unwrap();
		assert_eq!(result.topic_pattern(), topic_pattern);
	}

	#[test]
	fn test_parameter_not_found_error() {
		let pattern = create_pattern("sensors/{sensor_id}/data");
		let result = pattern.with_parameters([("nonexistent", "value")]);

		assert!(result.is_err());
		if let Err(TopicPatternError::WildcardUsage { usage }) = result {
			assert!(
				usage.contains("Parameter 'nonexistent' not found in pattern")
			);
		} else {
			panic!("Expected WildcardUsage error");
		}
	}

	#[test]
	fn test_mixed_valid_and_invalid_parameters() {
		let pattern = create_pattern("sensors/{sensor_id}/data/{type}");
		let params = [
			("sensor_id", "123"),
			("invalid_param", "value"), // This will cause error
		];

		let result = pattern.with_parameters(params);
		assert!(result.is_err());

		// Should fail on first invalid parameter
		if let Err(TopicPatternError::WildcardUsage { usage }) = result {
			assert!(
				usage
					.contains("Parameter 'invalid_param' not found in pattern")
			);
		}
	}

	#[test]
	fn test_anonymous_wildcards_are_not_substituted() {
		let pattern = create_pattern("sensors/+/data/{measurement}");
		let result = pattern
			.with_parameters([("measurement", "temperature")])
			.unwrap();

		// topic_pattern() preserves original template including anonymous wildcards
		assert_eq!(result.topic_pattern(), "sensors/+/data/{measurement}");
		// mqtt_pattern() substitutes named params, leaves anonymous wildcards
		assert_eq!(result.mqtt_pattern(), "sensors/+/data/temperature");
	}

	#[test]
	fn test_mixed_named_and_anonymous_wildcards() {
		let pattern =
			create_pattern("devices/{device_type}/+/status/{status_type}");
		let params = [("device_type", "sensor"), ("status_type", "online")];
		let result = pattern.with_parameters(params).unwrap();

		// topic_pattern() preserves original template
		assert_eq!(result.topic_pattern(), "devices/{device_type}/+/status/{status_type}");
		// mqtt_pattern() substitutes named params, preserves anonymous wildcards
		assert_eq!(result.mqtt_pattern(), "devices/sensor/+/status/online");
	}

	#[test]
	fn test_hash_wildcard_preservation() {
		let pattern = create_pattern("logs/{service}/#");
		let result = pattern.with_parameters([("service", "auth")]).unwrap();

		// topic_pattern() preserves original template
		assert_eq!(result.topic_pattern(), "logs/{service}/#");
		// mqtt_pattern() substitutes named params, preserves # wildcard
		assert_eq!(result.mqtt_pattern(), "logs/auth/#");
	}

	#[test]
	fn test_named_hash_wildcard_substitution() {
		let pattern = create_pattern("events/{category}/{details:#}");
		let result = pattern.with_parameters([("category", "alerts")]).unwrap();

		// topic_pattern() preserves original template including named hash wildcard
		assert_eq!(result.topic_pattern(), "events/{category}/{details:#}");
		// mqtt_pattern() substitutes named params, converts named hash to #
		assert_eq!(result.mqtt_pattern(), "events/alerts/#");
	}

	#[test]
	fn test_cache_strategy_preservation() {
		let pattern = TopicPatternPath::new_from_string(
			"sensors/{id}/data",
			CacheStrategy::Lru(std::num::NonZeroUsize::new(50).unwrap()),
		)
		.unwrap();

		let result = pattern.with_parameters([("id", "123")]).unwrap();

		// Cache strategy should be preserved
		match result.cache_strategy() {
			| CacheStrategy::Lru(size) => assert_eq!(size.get(), 50),
			| CacheStrategy::NoCache => panic!("Expected LRU cache strategy"),
		}
	}

	#[test]
	fn test_unicode_parameters() {
		let pattern = create_pattern("пристрої/{тип}/статус");
		let result = pattern.with_parameters([("тип", "сенсор")]).unwrap();

		assert_eq!(result.mqtt_pattern(), "пристрої/сенсор/статус");
	}

	#[test]
	fn test_special_characters_in_values() {
		let pattern = create_pattern("devices/{device_id}/data");
		let result = pattern
			.with_parameters([("device_id", "temp-sensor@home.local")])
			.unwrap();

		assert_eq!(
			result.mqtt_pattern(),
			"devices/temp-sensor@home.local/data"
		);
	}

	#[test]
	fn test_empty_string_parameter_value() {
		let pattern = create_pattern("prefix/{param}/suffix");
		let result = pattern.with_parameters([("param", "")]).unwrap();

		assert_eq!(result.mqtt_pattern(), "prefix//suffix");
	}

	#[test]
	fn test_pattern_with_no_wildcards() {
		let pattern = create_pattern("static/topic/path");
		let result = pattern.with_parameters([("nonexistent", "value")]);

		assert!(result.is_err());
		if let Err(TopicPatternError::WildcardUsage { usage }) = result {
			assert!(
				usage.contains("Parameter 'nonexistent' not found in pattern")
			);
		}
	}

	#[test]
	fn test_single_wildcard_pattern() {
		let pattern = create_pattern("{param}");
		let result = pattern.with_parameters([("param", "value")]).unwrap();

		// topic_pattern() preserves original template
		assert_eq!(result.topic_pattern(), "{param}");
		// mqtt_pattern() shows substituted result
		assert_eq!(result.mqtt_pattern(), "value");
	}

	#[test]
	fn test_parameter_order_independence() {
		let pattern = create_pattern("a/{first}/b/{second}/c");

		// Test different parameter orders
		let result1 = pattern.clone()
			.with_parameters([("first", "1"), ("second", "2")])
			.unwrap();
		let result2 = pattern
			.with_parameters([("second", "2"), ("first", "1")])
			.unwrap();

		// Both should produce the same MQTT pattern regardless of parameter order
		assert_eq!(result1.mqtt_pattern(), "a/1/b/2/c");
		assert_eq!(result2.mqtt_pattern(), "a/1/b/2/c");
		assert_eq!(result1.mqtt_pattern(), result2.mqtt_pattern());
	}

	fn str_to_topic_pattern_path(
		topic: &str,
	) -> Result<TopicPatternPath, TopicPatternError> {
		TopicPatternPath::new_from_string(topic, CacheStrategy::NoCache)
	}

	#[test]
	fn test_simple_string_pattern() {
		let result = str_to_topic_pattern_path("simple/path").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("simple")),
				TopicPatternItem::Str(Substr::from("path"))
			]
		);
	}

	#[test]
	fn test_pattern_with_star() {
		let result = str_to_topic_pattern_path("devices/+/status").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("devices")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("status"))
			]
		);
	}

	#[test]
	fn test_pattern_with_hash() {
		let result = str_to_topic_pattern_path("sensors/#").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("sensors")),
				TopicPatternItem::Hash(None)
			]
		);
	}

	#[test]
	fn test_pattern_with_both_wildcards() {
		let result = str_to_topic_pattern_path("home/+/device/#").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("home")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("device")),
				TopicPatternItem::Hash(None)
			]
		);
	}

	#[test]
	fn test_empty_string() {
		let result = str_to_topic_pattern_path("");
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), TopicPatternError::EmptyTopic);
	}

	#[test]
	fn test_only_wildcards() {
		let result_star = str_to_topic_pattern_path("+").unwrap();
		assert_eq!(result_star.segments(), &vec![TopicPatternItem::Plus(None)]);

		let result_hash = str_to_topic_pattern_path("#").unwrap();
		assert_eq!(result_hash.segments(), &vec![TopicPatternItem::Hash(None)]);
	}

	#[test]
	fn test_consecutive_separators() {
		let result = str_to_topic_pattern_path("topic//subtopic");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("topic")),
				TopicPatternItem::Str(Substr::from("")),
				TopicPatternItem::Str(Substr::from("subtopic"))
			]
		);
	}

	#[test]
	fn test_starting_with_separator() {
		let result = str_to_topic_pattern_path("/start");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("")),
				TopicPatternItem::Str(Substr::from("start"))
			]
		);
	}

	#[test]
	fn test_ending_with_separator() {
		let result = str_to_topic_pattern_path("end/");
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("end")),
				TopicPatternItem::Str(Substr::from(""))
			]
		);
	}

	#[test]
	fn test_invalid_hash_wildcard_position() {
		let result = str_to_topic_pattern_path("invalid/#/pattern");
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			TopicPatternError::HashPosition {
				pattern: "invalid/#/pattern".to_string()
			}
		);
	}

	#[test]
	fn test_very_long_pattern() {
		let long_pattern = "segment1/segment2/segment3/segment4/segment5/\
		                    segment6/segment7/segment8/segment9/segment10";
		let result = str_to_topic_pattern_path(long_pattern).unwrap();
		assert_eq!(result.segments().len(), 10);
	}

	#[test]
	fn test_unicode_characters() {
		let result = str_to_topic_pattern_path("пристрої/+/статус").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("пристрої")),
				TopicPatternItem::Plus(None),
				TopicPatternItem::Str(Substr::from("статус"))
			]
		);
	}

	#[test]
	fn test_special_characters() {
		let result =
			str_to_topic_pattern_path("device-123/status@home").unwrap();
		assert_eq!(
			result.segments(),
			&vec![
				TopicPatternItem::Str(Substr::from("device-123")),
				TopicPatternItem::Str(Substr::from("status@home"))
			]
		);
	}

	#[test]
	fn test_display_implementation() {
		// Test simple string pattern
		let path = str_to_topic_pattern_path("simple/path").unwrap();
		assert_eq!(path.to_string(), "simple/path");

		// Test with wildcards
		let path = str_to_topic_pattern_path("devices/+/status").unwrap();
		assert_eq!(path.to_string(), "devices/+/status");

		// Test with hash wildcard
		let path = str_to_topic_pattern_path("sensors/#").unwrap();
		assert_eq!(path.to_string(), "sensors/#");

		// Test empty path
		if let Err(err) = str_to_topic_pattern_path("") {
			assert_eq!(err, TopicPatternError::EmptyTopic);
		} else {
			panic!("Expected error for empty topic pattern");
		}

		// Test / path
		let path = str_to_topic_pattern_path("/").unwrap();
		assert_eq!(path.to_string(), "/");

		// Test finish / path
		let path = str_to_topic_pattern_path("device/").unwrap();
		assert_eq!(path.to_string(), "device/");

		// Test with consecutive separators
		let path = str_to_topic_pattern_path("topic//subtopic").unwrap();
		assert_eq!(path.to_string(), "topic//subtopic");
	}

	#[test]
	fn test_invalid_wildcards_format() {
		let result_double_star = str_to_topic_pattern_path("topic/++/subtopic");
		assert!(result_double_star.is_err());
		assert!(matches!(
			result_double_star.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));

		let result_double_hash = str_to_topic_pattern_path("topic/##");
		assert!(result_double_hash.is_err());
		assert!(matches!(
			result_double_hash.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));
	}

	#[test]
	fn test_wildcards_with_other_characters() {
		let result_star = str_to_topic_pattern_path("topic/a+b/subtopic");
		assert!(result_star.is_err());
		assert!(matches!(
			result_star.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));

		let result_hash = str_to_topic_pattern_path("topic/a#b");
		assert!(result_hash.is_err());
		assert!(matches!(
			result_hash.unwrap_err(),
			TopicPatternError::WildcardUsage { .. }
		));
	}
}

/// Integration tests with real-world patterns
mod integration_tests {
	use super::*;

	#[test]
	fn test_mqtt_sensor_reading_pattern() {
		let pattern = create_pattern("typed/{room}/pl/{sensor_id}/some/{temp}");
		let params = [
			("room", "kitchen"),
			("sensor_id", "floor_sensor"),
			("temp", "23.5"),
		];

		let result = pattern.with_parameters(params).unwrap();
		assert_eq!(
			result.mqtt_pattern(),
			"typed/kitchen/pl/floor_sensor/some/23.5"
		);
	}

	#[test]
	fn test_iot_device_telemetry_pattern() {
		let pattern = create_pattern(
			"iot/{building}/{floor}/devices/{device_type}/{device_id}/\
			 telemetry/{metric}",
		);
		let params = [
			("building", "office-a"),
			("floor", "3"),
			("device_type", "temperature"),
			("device_id", "temp-001"),
			("metric", "celsius"),
		];

		let result = pattern.with_parameters(params).unwrap();
		let expected =
			"iot/office-a/3/devices/temperature/temp-001/telemetry/celsius";
		assert_eq!(result.mqtt_pattern(), expected);
	}

	#[test]
	fn test_selective_filtering() {
		// Test case where we only want to filter specific sensors
		let pattern =
			create_pattern("sensors/{location}/{sensor_type}/{sensor_id}/data");

		// Filter only by sensor_type, leave others as wildcards
		let result = pattern
			.with_parameters([("sensor_type", "temperature")])
			.unwrap();

		// topic_pattern() preserves original template
		assert_eq!(
			result.topic_pattern(),
			"sensors/{location}/{sensor_type}/{sensor_id}/data"
		);
		// mqtt_pattern() shows partial substitution with + for unbound params
		assert_eq!(result.mqtt_pattern(), "sensors/+/temperature/+/data");
	}

	#[test]
	fn test_log_aggregation_pattern() {
		let pattern = create_pattern("logs/{service}/{level}/{details:#}");

		// Filter for specific service and log level
		let result = pattern
			.with_parameters([("service", "auth-service"), ("level", "error")])
			.unwrap();

		// topic_pattern() preserves original template
		assert_eq!(
			result.topic_pattern(),
			"logs/{service}/{level}/{details:#}"
		);
		// mqtt_pattern() substitutes bound params, converts named hash to #
		assert_eq!(result.mqtt_pattern(), "logs/auth-service/error/#");
	}
}
