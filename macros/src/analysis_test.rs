//! Tests for struct analysis and validation logic

use mqtt_typed_client_core::routing::subscription_manager::CacheStrategy;
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
fn create_topic_pattern(
	pattern: &str,
) -> mqtt_typed_client_core::topic::topic_pattern_path::TopicPatternPath {
	mqtt_typed_client_core::topic::topic_pattern_path::TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
		.expect("Invalid test pattern")
}

/// Helper to create a test struct
fn create_test_struct(fields: proc_macro2::TokenStream) -> syn::DeriveInput {
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
			let context = result.unwrap_or_else(|_| {
				panic!("Test '{}' should succeed but failed", test_case.name)
			});

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
fn test_topic_param_methods() {
	// Test get_publisher_param_name
	let named_param = TopicParam {
		name: Some("sensor_id".to_string()),
		wildcard_index: 0,
		struct_field_type: None,
	};
	assert_eq!(named_param.get_publisher_param_name(), "sensor_id");

	let anonymous_param = TopicParam {
		name: None,
		wildcard_index: 2,
		struct_field_type: None,
	};
	assert_eq!(anonymous_param.get_publisher_param_name(), "wildcard_2");

	// Test get_publisher_param_type
	let typed_param = TopicParam {
		name: Some("count".to_string()),
		wildcard_index: 0,
		struct_field_type: Some(syn::parse_quote!(u32)),
	};
	let param_type = typed_param.get_publisher_param_type();
	assert_eq!(quote::quote!(#param_type).to_string(), "u32");

	let untyped_param = TopicParam {
		name: Some("room".to_string()),
		wildcard_index: 1,
		struct_field_type: None,
	};
	let default_type = untyped_param.get_publisher_param_type();
	assert_eq!(quote::quote!(#default_type).to_string(), "& str");

	// Test is_anonymous
	assert!(!named_param.is_anonymous());
	assert!(anonymous_param.is_anonymous());

	// Test has_struct_field (field_type.is_some())
	assert!(named_param.struct_field_type.is_none());
	assert!(typed_param.struct_field_type.is_some());
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
		let named_params: Vec<_> =
			params.iter().filter(|p| p.name.is_some()).collect();

		assert_eq!(
			named_params.len(),
			test_case.expected_named.len(),
			"Test case {}: named param count mismatch for pattern '{}'",
			i,
			test_case.pattern
		);

		for (expected_name, expected_index) in test_case.expected_named {
			let found_param = params
				.iter()
				.find(|p| p.name.as_deref() == Some(expected_name));

			assert!(
				found_param.is_some(),
				"Test case {i}: missing parameter '{expected_name}'"
			);

			let param = found_param.unwrap();
			assert_eq!(
				param.wildcard_index, expected_index,
				"Test case {i}: wildcard index mismatch for '{expected_name}'"
			);
		}
	}
}

#[test]
fn test_field_type_mapping() {
	// Test when named parameter has corresponding struct field
	let pattern = create_topic_pattern("sensors/{sensor_id}/{room}/data");
	let test_struct: syn::DeriveInput = parse_quote! {
		struct TestStruct {
			sensor_id: u32,
			room: String,
			payload: f64,
		}
	};

	let context =
		StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();

	// Check that field types are mapped correctly
	let sensor_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("sensor_id"))
		.unwrap();
	assert!(sensor_param.struct_field_type.is_some());
	let sensor_type = sensor_param.struct_field_type.as_ref().unwrap();
	assert_eq!(quote::quote!(#sensor_type).to_string(), "u32");

	let room_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("room"))
		.unwrap();
	assert!(room_param.struct_field_type.is_some());
	let room_type = room_param.struct_field_type.as_ref().unwrap();
	assert_eq!(quote::quote!(#room_type).to_string(), "String");
}

#[test]
fn test_named_param_without_struct_field() {
	// Test when pattern has named parameter but struct doesn't have corresponding field
	let pattern =
		create_topic_pattern("sensors/{sensor_id}/{missing_field}/data");
	let test_struct: syn::DeriveInput = parse_quote! {
		struct TestStruct {
			sensor_id: u32,
			// missing_field is not defined
			payload: String,
		}
	};

	let context =
		StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();

	// sensor_id should have field type
	let sensor_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("sensor_id"))
		.unwrap();
	assert!(sensor_param.struct_field_type.is_some());

	// missing_field should not have field type
	let missing_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("missing_field"))
		.unwrap();
	assert!(missing_param.struct_field_type.is_none());
	assert_eq!(
		missing_param.get_publisher_param_type(),
		syn::parse_quote!(&str)
	);
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
					mqtt_typed_client_core::topic::topic_match::TopicMatch,
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
				error
					.to_string()
					.contains("must be of type Arc<TopicMatch>"),
				"Test '{}': wrong error message: {}",
				test_case.name,
				error
			);
		}
	}
}

#[test]
fn test_complex_wildcard_patterns() {
	// Test mixed named, anonymous, and hash wildcards
	let test_cases = vec![
		(
			"devices/+/{device_id}/sensors/+/{sensor_type}/#",
			vec![("device_id", 1), ("sensor_type", 3)], // named params with indices
			5, // total wildcards: +, {device_id}, +, {sensor_type}, #
		),
		(
			"buildings/{building}/+/floors/{floor}/+/rooms/+",
			vec![("building", 0), ("floor", 2)],
			5, // {building}, +, {floor}, +, +
		),
		(
			"+/{param1}/+/{param2}/+",
			vec![("param1", 1), ("param2", 3)],
			5, // +, {param1}, +, {param2}, +
		),
	];

	for (pattern_str, expected_named, expected_total) in test_cases {
		let pattern = create_topic_pattern(pattern_str);
		let field_types = std::collections::HashMap::new();
		let params = TopicParam::build_topic_params(&pattern, &field_types);

		assert_eq!(
			params.len(),
			expected_total,
			"Pattern '{pattern_str}': wrong total wildcard count"
		);

		let named_count = expected_named.len();
		// Check named parameters
		for (expected_name, expected_index) in expected_named {
			let found = params
				.iter()
				.find(|p| p.name.as_deref() == Some(expected_name))
				.unwrap();
			assert_eq!(
				found.wildcard_index, expected_index,
				"Pattern '{pattern_str}': wrong index for '{expected_name}'"
			);
		}

		// Count anonymous wildcards
		let anonymous_count =
			params.iter().filter(|p| p.is_anonymous()).count();

		assert_eq!(
			anonymous_count + named_count,
			expected_total,
			"Pattern '{pattern_str}': anonymous + named != total"
		);
	}
}

#[test]
fn test_has_special_fields() {
	// Test has_special_fields method
	let pattern = create_topic_pattern("test/{param}");

	// No special fields
	let struct1: syn::DeriveInput = parse_quote! {
		struct Test1 { param: String }
	};
	let context1 = StructAnalysisContext::analyze(&struct1, &pattern).unwrap();
	assert!(!context1.has_special_fields());

	// With payload
	let struct2: syn::DeriveInput = parse_quote! {
		struct Test2 { param: String, payload: Vec<u8> }
	};
	let context2 = StructAnalysisContext::analyze(&struct2, &pattern).unwrap();
	assert!(context2.has_special_fields());

	// With topic field
	let struct3: syn::DeriveInput = parse_quote! {
		struct Test3 { param: String, topic: Arc<TopicMatch> }
	};
	let context3 = StructAnalysisContext::analyze(&struct3, &pattern).unwrap();
	assert!(context3.has_special_fields());

	// With both
	let struct4: syn::DeriveInput = parse_quote! {
		struct Test4 { param: String, payload: String, topic: Arc<TopicMatch> }
	};
	let context4 = StructAnalysisContext::analyze(&struct4, &pattern).unwrap();
	assert!(context4.has_special_fields());
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
fn test_custom_field_types() {
	// Test various custom field types in topic parameters
	let pattern =
		create_topic_pattern("api/{version}/{user_id}/{category}/{item_id}");
	let test_struct: syn::DeriveInput = parse_quote! {
		struct ApiRequest {
			version: String,
			user_id: u64,
			category: std::borrow::Cow<'static, str>,
			item_id: uuid::Uuid,
			payload: serde_json::Value,
		}
	};

	let context =
		StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();

	// Verify all parameters have correct field types
	let types = [
		("version", "String"),
		("user_id", "u64"),
		("category", "std :: borrow :: Cow < 'static , str >"),
		("item_id", "uuid :: Uuid"),
	];

	for (param_name, expected_type) in types {
		let param = context
			.topic_params
			.iter()
			.find(|p| p.name.as_deref() == Some(param_name))
			.unwrap();
		assert!(param.struct_field_type.is_some());
		let field_type = param.struct_field_type.as_ref().unwrap();
		let actual_type = quote::quote!(#field_type).to_string();
		assert_eq!(
			actual_type, expected_type,
			"Wrong type for parameter '{param_name}'"
		);
	}

	#[cfg(test)]
	{
		assert_eq!(context.param_count(), 4);
	}
	assert!(context.payload_type.is_some());
}

#[test]
fn test_publisher_param_generation() {
	// Test parameter name/type generation for publisher methods
	let pattern = create_topic_pattern("sensors/+/{device_id}/+/{room}/data");
	let test_struct: syn::DeriveInput = parse_quote! {
		struct SensorData {
			device_id: u32,
			// room field missing - should use &str
			payload: f64,
		}
	};

	let context =
		StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();
	#[cfg(test)]
	{
		assert_eq!(context.param_count(), 4); // +, {device_id}, +, {room}
	}

	// Check anonymous wildcard names
	let anonymous_params: Vec<_> = context
		.topic_params
		.iter()
		.filter(|p| p.is_anonymous())
		.collect();
	assert_eq!(anonymous_params.len(), 2);
	assert_eq!(anonymous_params[0].get_publisher_param_name(), "wildcard_0"); // index 0
	assert_eq!(anonymous_params[1].get_publisher_param_name(), "wildcard_2"); // index 2

	// Check named parameter with field
	let device_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("device_id"))
		.unwrap();
	assert_eq!(device_param.get_publisher_param_name(), "device_id");
	let device_type = device_param.get_publisher_param_type();
	assert_eq!(quote::quote!(#device_type).to_string(), "u32");

	// Check named parameter without field
	let room_param = context
		.topic_params
		.iter()
		.find(|p| p.name.as_deref() == Some("room"))
		.unwrap();
	assert_eq!(room_param.get_publisher_param_name(), "room");
	let room_type = room_param.get_publisher_param_type();
	assert_eq!(quote::quote!(#room_type).to_string(), "& str");
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
			.contains("mqtt_topic can only be applied to structs")
	);

	// Test tuple struct
	let test_tuple: syn::DeriveInput = parse_quote! {
		struct TestStruct(u32, String);
	};
	let result = StructAnalysisContext::analyze(&test_tuple, &pattern);
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("named fields"));
}
