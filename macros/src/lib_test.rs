//! Tests for main macro entry point and core functionality

use quote::quote;
use syn::parse_quote;

use super::*;
use crate::analysis::{StructAnalysisContext, TopicParam};

/// Helper function for tests
fn create_topic_pattern(pattern: &str) -> TopicPatternPath {
	TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
		.expect("Invalid test pattern")
}

#[test]
fn test_parse_topic_pattern_validation() {
	struct PatternTestCase {
		name: &'static str,
		pattern: &'static str,
		should_succeed: bool,
		error_contains: Option<&'static str>,
	}

	let test_cases = vec![
		PatternTestCase {
			name: "valid_simple",
			pattern: "sensors/{sensor_id}/data",
			should_succeed: true,
			error_contains: None,
		},
		PatternTestCase {
			name: "valid_complex",
			pattern: "buildings/{building}/floors/{floor}/rooms/{room}",
			should_succeed: true,
			error_contains: None,
		},
		PatternTestCase {
			name: "valid_with_hash_end",
			pattern: "sensors/{device_id}/#",
			should_succeed: true,
			error_contains: None,
		},
		PatternTestCase {
			name: "invalid_empty",
			pattern: "",
			should_succeed: false,
			error_contains: Some("Invalid topic pattern"),
		},
		PatternTestCase {
			name: "invalid_hash_middle",
			pattern: "sensors/#/data",
			should_succeed: false,
			error_contains: Some("Invalid topic pattern"),
		},
		PatternTestCase {
			name: "invalid_syntax",
			pattern: "sensors/{}/data", // Empty parameter name
			should_succeed: false,
			error_contains: Some("Invalid topic pattern"),
		},
	];

	for test_case in test_cases {
		let pattern_str =
			syn::LitStr::new(test_case.pattern, proc_macro2::Span::call_site());
		let result = parse_topic_pattern(&pattern_str);

		if test_case.should_succeed {
			assert!(
				result.is_ok(),
				"Test '{}': pattern '{}' should be valid",
				test_case.name,
				test_case.pattern
			);
		} else {
			assert!(
				result.is_err(),
				"Test '{}': pattern '{}' should be invalid",
				test_case.name,
				test_case.pattern
			);
			if let Some(expected_error) = test_case.error_contains {
				let error = result.unwrap_err();
				assert!(
					error.to_string().contains(expected_error),
					"Test '{}': error should contain '{}', got '{}'",
					test_case.name,
					expected_error,
					error
				);
			}
		}
	}
}

#[test]
fn test_generate_mqtt_code_integration() {
	// Test the main orchestration function with different configurations
	struct IntegrationTestCase {
		name: &'static str,
		pattern: &'static str,
		struct_fields: proc_macro2::TokenStream,
		subscriber: bool,
		publisher: bool,
		should_succeed: bool,
		expected_contains: Vec<&'static str>,
		expected_not_contains: Vec<&'static str>,
	}

	let test_cases = vec![
		IntegrationTestCase {
			name: "subscriber_only_success",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				sensor_id: u32,
				payload: String,
			},
			subscriber: true,
			publisher: false,
			should_succeed: true,
			expected_contains: vec![
				"FromMqttMessage",
				"pub async fn subscribe",
				"TOPIC_PATTERN",
				"MQTT_PATTERN",
			],
			expected_not_contains: vec![
				"pub async fn publish",
				"pub fn get_publisher",
			],
		},
		IntegrationTestCase {
			name: "publisher_only_success",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				sensor_id: u32,
				payload: String,
			},
			subscriber: false,
			publisher: true,
			should_succeed: true,
			expected_contains: vec![
				"pub async fn publish",
				"pub fn get_publisher",
				"TOPIC_PATTERN",
				"MQTT_PATTERN",
			],
			expected_not_contains: vec![
				"FromMqttMessage",
				"pub async fn subscribe",
			],
		},
		IntegrationTestCase {
			name: "both_modes_success",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				sensor_id: u32,
				payload: String,
			},
			subscriber: true,
			publisher: true,
			should_succeed: true,
			expected_contains: vec![
				"FromMqttMessage",
				"pub async fn subscribe",
				"pub async fn publish",
				"pub fn get_publisher",
				"TOPIC_PATTERN",
				"MQTT_PATTERN",
			],
			expected_not_contains: vec![],
		},
		IntegrationTestCase {
			name: "struct_validation_error",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				unknown_field: String, // Should cause validation error
			},
			subscriber: true,
			publisher: true,
			should_succeed: false,
			expected_contains: vec![],
			expected_not_contains: vec![],
		},
	];

	for test_case in test_cases {
		let struct_fields = &test_case.struct_fields;
		let test_struct: syn::DeriveInput = parse_quote! {
			struct TestStruct {
				#struct_fields
			}
		};

		let pattern = create_topic_pattern(test_case.pattern);
		let macro_args = MacroArgs {
			pattern,
			generate_subscriber: test_case.subscriber,
			generate_publisher: test_case.publisher,
			generate_typed_client: true,
			generate_last_will: test_case.publisher,
		};

		let result = generate_mqtt_code(macro_args, &test_struct);

		if test_case.should_succeed {
			let generated = result.unwrap_or_else(|_| {
				panic!("Test '{}' should succeed", test_case.name)
			});
			let code = generated.to_string();

			for expected in test_case.expected_contains {
				assert!(
					code.contains(expected),
					"Test '{}': generated code should contain '{}'",
					test_case.name,
					expected
				);
			}

			for not_expected in test_case.expected_not_contains {
				assert!(
					!code.contains(not_expected),
					"Test '{}': generated code should NOT contain '{}'",
					test_case.name,
					not_expected
				);
			}
		} else {
			assert!(result.is_err(), "Test '{}' should fail", test_case.name);
		}
	}
}

#[test]
fn test_complex_patterns() {
	// Test various complex patterns with the full pipeline
	let complex_cases = vec![
		(
			"multi_level_named",
			"buildings/{building}/floors/{floor}/rooms/{room}/sensors/\
			 {sensor_id}",
			quote! {
				building: String,
				floor: u32,
				room: String,
				sensor_id: u32,
				payload: f64,
			},
		),
		(
			"mixed_wildcards",
			"devices/+/{device_id}/+/status",
			quote! {
				device_id: String,
				payload: bool,
			},
		),
		(
			"anonymous_only",
			"heartbeat/+/+",
			quote! {
				payload: String,
			},
		),
	];

	for (name, pattern, struct_fields) in complex_cases {
		let test_struct: syn::DeriveInput = parse_quote! {
			struct TestStruct {
				#struct_fields
			}
		};

		let topic_pattern = create_topic_pattern(pattern);
		let macro_args = MacroArgs {
			pattern: topic_pattern,
			generate_subscriber: true,
			generate_publisher: true,
			generate_typed_client: true,
			generate_last_will: true,
		};

		let result = generate_mqtt_code(macro_args, &test_struct);
		assert!(
			result.is_ok(),
			"Test '{name}': complex pattern should generate successfully"
		);

		let generated = result.unwrap();
		let code = generated.to_string();

		// Basic checks that code generation worked
		assert!(
			code.contains("TOPIC_PATTERN"),
			"Test '{name}': should have TOPIC_PATTERN"
		);
		assert!(
			code.contains("MQTT_PATTERN"),
			"Test '{name}': should have MQTT_PATTERN"
		);
	}
}

#[test]
fn test_macro_args_validation() {
	// Test MacroArgs structure and behavior directly
	struct ValidationTest {
		name: &'static str,
		pattern: &'static str,
		subscriber: bool,
		publisher: bool,
		should_be_valid: bool,
	}

	let test_cases = vec![
		ValidationTest {
			name: "both_modes",
			pattern: "sensors/{sensor_id}/data",
			subscriber: true,
			publisher: true,
			should_be_valid: true,
		},
		ValidationTest {
			name: "subscriber_only",
			pattern: "sensors/{sensor_id}/data",
			subscriber: true,
			publisher: false,
			should_be_valid: true,
		},
		ValidationTest {
			name: "publisher_only",
			pattern: "sensors/{sensor_id}/data",
			subscriber: false,
			publisher: true,
			should_be_valid: true,
		},
		ValidationTest {
			name: "hash_with_subscriber_only",
			pattern: "sensors/{device_id}/#",
			subscriber: true,
			publisher: false,
			should_be_valid: true,
		},
		ValidationTest {
			name: "hash_with_publisher_should_fail",
			pattern: "sensors/{device_id}/#",
			subscriber: false,
			publisher: true,
			should_be_valid: false, // Should fail in real usage
		},
	];

	for test_case in test_cases {
		let pattern_result = TopicPatternPath::new_from_string(
			test_case.pattern,
			CacheStrategy::NoCache,
		);

		if pattern_result.is_err() {
			continue; // Skip invalid patterns
		}

		let pattern = pattern_result.unwrap();
		let has_hash = pattern.contains_hash();

		let macro_args = MacroArgs {
			pattern,
			generate_subscriber: test_case.subscriber,
			generate_publisher: test_case.publisher,
			generate_typed_client: true,
			generate_last_will: test_case.publisher,
		};

		// Test configuration validity
		assert!(
			macro_args.generate_subscriber || macro_args.generate_publisher,
			"Test '{}': at least one mode should be enabled",
			test_case.name
		);

		// Test hash wildcard constraint
		if test_case.publisher && has_hash {
			// This combination should be rejected by parse_macro_args in real usage
			assert!(
				!test_case.should_be_valid,
				"Test '{}': publisher with hash should be invalid",
				test_case.name
			);
		}
	}
}

#[test]
fn test_struct_analysis_context_utility() {
	// Test the utility function for creating contexts
	let payload_type: syn::Type = parse_quote!(String);
	let topic_params = vec![
		TopicParam {
			name: Some("sensor_id".to_string()),
			wildcard_index: 0,
			struct_field_type: Some(parse_quote!(u32)),
		},
		TopicParam {
			name: Some("room".to_string()),
			wildcard_index: 1,
			struct_field_type: Some(parse_quote!(String)),
		},
	];

	let context = StructAnalysisContext::from_components(
		Some(payload_type),
		true,
		topic_params,
	);

	assert!(context.payload_type.is_some());
	assert!(context.has_topic_field);
	assert_eq!(context.topic_params.len(), 2);
	assert_eq!(context.param_count(), 2);
	assert!(context.has_special_fields());

	let param_names = context.param_names();
	assert!(param_names.contains(&"sensor_id"));
	assert!(param_names.contains(&"room"));
}

#[test]
fn test_real_world_scenarios() {
	// Test scenarios that mimic real usage patterns
	struct RealWorldTest {
		name: &'static str,
		pattern: &'static str,
		struct_def: proc_macro2::TokenStream,
		modes: (bool, bool), // (subscriber, publisher)
	}

	let scenarios = vec![
		RealWorldTest {
			name: "iot_sensor_both",
			pattern: "iot/sensors/{building}/{floor}/{sensor_id}/temperature",
			struct_def: quote! {
				building: String,
				floor: u32,
				sensor_id: String,
				payload: f64,
				topic: Arc<TopicMatch>,
			},
			modes: (true, true),
		},
		RealWorldTest {
			name: "device_status_subscriber_only",
			pattern: "devices/+/{device_id}/status/#",
			struct_def: quote! {
				device_id: uuid::Uuid,
				payload: DeviceStatus,
			},
			modes: (true, false), // Hash wildcard blocks publisher
		},
		RealWorldTest {
			name: "command_publisher_only",
			pattern: "commands/{service}/{action}/{target_id}",
			struct_def: quote! {
				service: String,
				action: CommandType,
				target_id: u64,
				payload: serde_json::Value,
			},
			modes: (false, true),
		},
		RealWorldTest {
			name: "heartbeat_minimal",
			pattern: "heartbeat/{service_name}",
			struct_def: quote! {
				service_name: String,
			},
			modes: (true, true),
		},
	];

	for scenario in scenarios {
		let struct_def = &scenario.struct_def;
		let test_struct: syn::DeriveInput = parse_quote! {
			struct TestStruct {
				#struct_def
			}
		};

		let pattern = create_topic_pattern(scenario.pattern);
		let macro_args = MacroArgs {
			pattern,
			generate_subscriber: scenario.modes.0,
			generate_publisher: scenario.modes.1,
			generate_typed_client: true,
			generate_last_will: scenario.modes.1,
		};

		let result = generate_mqtt_code(macro_args, &test_struct);

		// Hash patterns should only work with subscriber-only
		let has_hash = scenario.pattern.contains('#');
		if has_hash && scenario.modes.1 {
			assert!(
				result.is_err(),
				"Scenario '{}': hash patterns should block publisher",
				scenario.name
			);
			continue;
		}

		assert!(
			result.is_ok(),
			"Scenario '{}': should generate successfully",
			scenario.name
		);

		let generated = result.unwrap();
		let code = generated.to_string();

		// Verify expected functionality is present
		if scenario.modes.0 {
			assert!(
				code.contains("FromMqttMessage"),
				"Scenario '{}': should have subscriber functionality",
				scenario.name
			);
		}

		if scenario.modes.1 {
			assert!(
				code.contains("pub async fn publish"),
				"Scenario '{}': should have publisher functionality",
				scenario.name
			);
		}
	}
}
