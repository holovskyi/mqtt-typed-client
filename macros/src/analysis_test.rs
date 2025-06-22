//! Tests for struct analysis and validation logic

use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
use quote::quote;
use syn::parse_quote;

use super::analysis::*;

/// Test case for struct analysis
struct AnalysisTestCase {
	name: &'static str,
	pattern: &'static str,
	struct_fields: proc_macro2::TokenStream,
	expected_result: AnalysisResult,
}

/// Expected result of analysis
#[derive(Debug, PartialEq)]
enum AnalysisResult {
	Success {
		param_count: usize,
		has_payload: bool,
		has_topic_field: bool,
		param_names: Vec<&'static str>,
	},
	Error {
		error_contains: &'static str,
	},
}

/// Helper to create a topic pattern for testing
fn create_topic_pattern(pattern: &str) -> mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath {
	mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
		.expect("Invalid test pattern")
}

/// Helper to create a test struct
fn create_test_struct(
	fields: proc_macro2::TokenStream,
) -> syn::DeriveInput {
	parse_quote! {
		struct TestStruct {
			#fields
		}
	}
}

/// Run a single analysis test case
fn run_analysis_test(test_case: AnalysisTestCase) {
	let pattern = create_topic_pattern(test_case.pattern);
	let test_struct = create_test_struct(test_case.struct_fields);
	let result = StructAnalysisContext::analyze(&test_struct, &pattern);

	match test_case.expected_result {
		| AnalysisResult::Success {
			param_count,
			has_payload,
			has_topic_field,
			param_names,
		} => {
			let context = result.expect(&format!(
				"Test '{}' should succeed but failed",
				test_case.name
			));

			assert_eq!(
				context.param_count(),
				param_count,
				"Test '{}': param count mismatch",
				test_case.name
			);
			assert_eq!(
				context.has_special_fields(),
				has_payload || has_topic_field,
				"Test '{}': special fields mismatch",
				test_case.name
			);
			assert_eq!(
				context.payload_type.is_some(),
				has_payload,
				"Test '{}': payload presence mismatch",
				test_case.name
			);
			assert_eq!(
				context.has_topic_field, has_topic_field,
				"Test '{}': topic field presence mismatch",
				test_case.name
			);

			let actual_names = context.param_names();
			assert_eq!(
				actual_names.len(),
				param_names.len(),
				"Test '{}': param names count mismatch",
				test_case.name
			);
			for expected_name in param_names {
				assert!(
					actual_names.contains(&expected_name),
					"Test '{}': missing parameter '{}'",
					test_case.name,
					expected_name
				);
			}
		}
		| AnalysisResult::Error { error_contains } => {
			let error = result.expect_err(&format!(
				"Test '{}' should fail but succeeded",
				test_case.name
			));
			let error_msg = error.to_string();
			assert!(
				error_msg.contains(error_contains),
				"Test '{}': error message '{}' should contain '{}'",
				test_case.name,
				error_msg,
				error_contains
			);
		}
	}
}

#[test]
fn test_topic_param_build() {
	struct ParamTestCase {
		pattern: &'static str,
		expected_named: Vec<(&'static str, usize)>, // (name, wildcard_index)
		expected_total_wildcards: usize,
	}

	let test_cases = vec![
		ParamTestCase {
			pattern: "sensors/{sensor_id}/+/{room}/data",
			expected_named: vec![("sensor_id", 0), ("room", 2)],
			expected_total_wildcards: 3, // {sensor_id}, +, {room}
		},
		ParamTestCase {
			pattern: "home/+/{device_id}/status/#",
			expected_named: vec![("device_id", 1)],
			expected_total_wildcards: 3, // +, {device_id}, #
		},
		ParamTestCase {
			pattern: "simple/+/topic",
			expected_named: vec![],
			expected_total_wildcards: 1, // +
		},
		ParamTestCase {
			pattern: "{a}/{b}/{c}",
			expected_named: vec![("a", 0), ("b", 1), ("c", 2)],
			expected_total_wildcards: 3,
		},
	];

	for (i, test_case) in test_cases.into_iter().enumerate() {
		let pattern = create_topic_pattern(test_case.pattern);
		let field_types = std::collections::HashMap::new();
		let params = TopicParam::build_topic_params(&pattern, &field_types);

		// Check total wildcard count
		assert_eq!(
			params.len(),
			test_case.expected_total_wildcards,
			"Test case {}: total wildcard count mismatch for pattern '{}'",
			i,
			test_case.pattern
		);

		// Check named parameters
		let named_params: Vec<_> = params.iter()
			.filter(|p| p.name.is_some())
			.collect();
		
		assert_eq!(
			named_params.len(),
			test_case.expected_named.len(),
			"Test case {}: named param count mismatch for pattern '{}'",
			i,
			test_case.pattern
		);

		for (expected_name, expected_index) in test_case.expected_named {
			let found_param = params.iter().find(|p| {
				p.name.as_ref().map(|n| n.as_str()) == Some(expected_name)
			});
			
			assert!(found_param.is_some(), 
				"Test case {}: missing parameter '{}'", i, expected_name);
				
			let param = found_param.unwrap();
			assert_eq!(
				param.wildcard_index, expected_index,
				"Test case {}: wildcard index mismatch for '{}'",
				i, expected_name
			);
		}
	}
}

#[test]
fn test_topic_field_validation() {
	struct TypeTestCase {
		name: &'static str,
		type_tokens: proc_macro2::TokenStream,
		should_be_valid: bool,
	}

	let test_cases = vec![
		// Valid types - should pass validation
		TypeTestCase {
			name: "simple_arc_topic_match",
			type_tokens: quote!(Arc<TopicMatch>),
			should_be_valid: true,
		},
		TypeTestCase {
			name: "fully_qualified_arc_topic_match",
			type_tokens: quote!(
				std::sync::Arc<
					mqtt_typed_client::topic::topic_match::TopicMatch,
				>
			),
			should_be_valid: true,
		},
		// Invalid types - should fail validation
		TypeTestCase {
			name: "arc_with_wrong_inner_type",
			type_tokens: quote!(Arc<String>),
			should_be_valid: false,
		},
		TypeTestCase {
			name: "topic_match_without_arc",
			type_tokens: quote!(TopicMatch),
			should_be_valid: false,
		},
		TypeTestCase {
			name: "vec_with_topic_match",
			type_tokens: quote!(Vec<TopicMatch>),
			should_be_valid: false,
		},
		TypeTestCase {
			name: "primitive_type",
			type_tokens: quote!(u32),
			should_be_valid: false,
		},
	];

	// Test by creating structs with topic field and checking validation
	for test_case in test_cases {
		let field_type = test_case.type_tokens;
		let test_struct: syn::DeriveInput = parse_quote! {
			struct TestStruct {
				topic: #field_type,
			}
		};

		let pattern = create_topic_pattern("sensors/+/data");
		let result = StructAnalysisContext::analyze(&test_struct, &pattern);

		if test_case.should_be_valid {
			assert!(
				result.is_ok(),
				"Test '{}': expected success but got error: {:?}",
				test_case.name,
				result.err()
			);
			let context = result.unwrap();
			assert!(context.has_topic_field, "Should detect topic field");
		} else {
			assert!(
				result.is_err(),
				"Test '{}': expected error but got success",
				test_case.name
			);
			let error = result.unwrap_err();
			assert!(
				error.to_string().contains("must be of type Arc<TopicMatch>"),
				"Test '{}': wrong error message: {}",
				test_case.name,
				error
			);
		}
	}
}

#[test]
fn test_comprehensive_analysis() {
	let test_cases = vec![
		// Success cases
		AnalysisTestCase {
			name: "basic_sensor_reading",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				sensor_id: u32,
				payload: String,
			},
			expected_result: AnalysisResult::Success {
				param_count: 1,
				has_payload: true,
				has_topic_field: false,
				param_names: vec!["sensor_id"],
			},
		},
		AnalysisTestCase {
			name: "multi_param_with_topic",
			pattern: "sensors/{sensor_id}/{room}/data",
			struct_fields: quote! {
				sensor_id: u32,
				room: String,
				payload: Vec<u8>,
				topic: Arc<TopicMatch>,
			},
			expected_result: AnalysisResult::Success {
				param_count: 2,
				has_payload: true,
				has_topic_field: true,
				param_names: vec!["sensor_id", "room"],
			},
		},
		AnalysisTestCase {
			name: "no_payload_field",
			pattern: "heartbeat/{service}",
			struct_fields: quote! {
				service: String,
			},
			expected_result: AnalysisResult::Success {
				param_count: 1,
				has_payload: false,
				has_topic_field: false,
				param_names: vec!["service"],
			},
		},
		AnalysisTestCase {
			name: "empty_struct_anonymous_wildcards",
			pattern: "sensors/+/data",
			struct_fields: quote! {},
			expected_result: AnalysisResult::Success {
				param_count: 1,
				has_payload: false,
				has_topic_field: false,
				param_names: vec![],
			},
		},
		// Error cases
		AnalysisTestCase {
			name: "unknown_field_error",
			pattern: "sensors/{sensor_id}/data",
			struct_fields: quote! {
				sensor_id: u32,
				unknown_field: String,
			},
			expected_result: AnalysisResult::Error {
				error_contains: "Unknown fields",
			},
		},
		AnalysisTestCase {
			name: "invalid_topic_field_type",
			pattern: "sensors/+/data",
			struct_fields: quote! {
				topic: String,
			},
			expected_result: AnalysisResult::Error {
				error_contains: "must be of type Arc<TopicMatch>",
			},
		},
	];

	for test_case in test_cases {
		run_analysis_test(test_case);
	}
}

#[test]
fn test_invalid_struct_types() {
	let pattern = create_topic_pattern("test/+");

	// Test enum
	let test_enum: syn::DeriveInput = parse_quote! {
		enum TestEnum {
			Variant1,
			Variant2,
		}
	};
	let result = StructAnalysisContext::analyze(&test_enum, &pattern);
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("can only be applied to structs")
	);

	// Test tuple struct
	let test_tuple: syn::DeriveInput = parse_quote! {
		struct TestStruct(u32, String);
	};
	let result = StructAnalysisContext::analyze(&test_tuple, &pattern);
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("named fields"));
}
