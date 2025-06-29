//! Tests for code generation logic

use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
use quote::quote;
use syn::parse_quote;

use super::MacroArgs;
use super::analysis::StructAnalysisContext;
use super::codegen::CodeGenerator;

/// Test case for code generation
struct CodegenTestCase {
	name: &'static str,
	pattern: &'static str,
	struct_fields: proc_macro2::TokenStream,
	generation_config: GenerationConfig,
	expected_checks: Vec<CodeCheck>,
}

/// Generation configuration for tests
#[derive(Debug, Clone)]
enum GenerationConfig {
	Both,
	SubscriberOnly,
	PublisherOnly,
}

/// What to check in generated code
#[derive(Debug, Clone)]
enum CodeCheck {
	/// Check that a parameter is extracted with correct index
	ParamExtraction {
		param_name: &'static str,
		index: usize,
	},
	/// Check that field assignment exists
	FieldAssignment(&'static str),
	/// Check that constant is defined with correct value
	Constant {
		name: &'static str,
		value: &'static str,
	},
	/// Check that method exists
	Method(&'static str),
	/// Check trait implementation
	TraitImpl(&'static str),
	/// Check payload type in generated code
	PayloadType(&'static str),
	/// Check publisher method parameter
	PublisherParam {
		param_name: &'static str,
		param_type: &'static str,
	},
	/// Check format string generation
	FormatString(&'static str),
	/// Check that something does NOT exist
	NotPresent(&'static str),
}

/// Helper to create a topic pattern for testing
fn create_topic_pattern(
	pattern: &str,
) -> mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath {
	mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
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

/// Helper to create MacroArgs with generation configuration
fn create_macro_args(pattern: &str, config: GenerationConfig) -> MacroArgs {
	let topic_pattern = create_topic_pattern(pattern);
	let (generate_subscriber, generate_publisher) = match config {
		| GenerationConfig::Both => (true, true),
		| GenerationConfig::SubscriberOnly => (true, false),
		| GenerationConfig::PublisherOnly => (false, true),
	};

	MacroArgs {
		pattern: topic_pattern,
		generate_subscriber,
		generate_publisher,
		generate_typed_client: true,  // Enable by default
	}
}

/// Helper to analyze and create a code generator
fn create_generator(
	struct_fields: proc_macro2::TokenStream,
	pattern: &str,
	config: GenerationConfig,
) -> (CodeGenerator, syn::DeriveInput) {
	let test_struct = create_test_struct(struct_fields);
	let macro_args = create_macro_args(pattern, config);
	let context =
		StructAnalysisContext::analyze(&test_struct, &macro_args.pattern)
			.expect("Analysis should succeed");
	let generator = CodeGenerator::new(context, macro_args);

	(generator, test_struct)
}

/// Run checks against generated code
fn verify_generated_code(code: &str, checks: Vec<CodeCheck>, test_name: &str) {
	for check in checks {
		match check {
			| CodeCheck::ParamExtraction { param_name, index } => {
				let has_extract_call = code.contains("extract_topic_parameter");
				let has_param_name =
					code.contains(&format!("\"{}\"", param_name));
				let has_index = code.contains(&format!("&topic, {}, ", index))
					|| code.contains(&format!("&topic, {}usize,", index))
					|| code.contains(&format!("& topic , {} ,", index))
					|| code.contains(&format!("& topic , {}usize ,", index));

				let found = has_extract_call && has_param_name && has_index;
				assert!(
					found,
					"Test '{}': missing parameter extraction for '{}' at \
					 index {}\nGenerated code: {}",
					test_name, param_name, index, code
				);
			}
			| CodeCheck::FieldAssignment(field) => {
				let patterns =
					[format!("{} ,", field), format!("{},", field)];
				let found =
					patterns.iter().any(|pattern| code.contains(pattern));
				assert!(
					found,
					"Test '{}': missing field assignment for '{}'\nGenerated \
					 code: {}",
					test_name, field, code
				);
			}
			| CodeCheck::Constant { name, value } => {
				let patterns = [
					format!(
						"pub const {} : & 'static str = \"{}\" ;",
						name, value
					),
					format!(
						"pub const {}: &'static str = \"{}\";",
						name, value
					),
					format!(
						"pub const {} : &'static str = \"{}\" ;",
						name, value
					),
				];
				let found =
					patterns.iter().any(|pattern| code.contains(pattern));
				assert!(
					found,
					"Test '{}': missing constant '{}' with value \
					 '{}'\nGenerated code: {}",
					test_name, name, value, code
				);
			}
			| CodeCheck::Method(method_name) => {
				let patterns = [
					format!("pub async fn {}", method_name),
					format!("pub async fn {}(", method_name),
					format!("pub fn {}", method_name),
					format!("pub fn {}(", method_name),
				];
				let found =
					patterns.iter().any(|pattern| code.contains(pattern));
				assert!(
					found,
					"Test '{}': missing method '{}'\nGenerated code: {}",
					test_name, method_name, code
				);
			}
			| CodeCheck::TraitImpl(trait_name) => {
				assert!(
					code.contains(trait_name),
					"Test '{}': missing trait implementation for \
					 '{}'\nGenerated code: {}",
					test_name,
					trait_name,
					code
				);
			}
			| CodeCheck::PayloadType(type_name) => {
				assert!(
					code.contains(type_name),
					"Test '{}': missing payload type '{}'\nGenerated code: {}",
					test_name,
					type_name,
					code
				);
			}
			| CodeCheck::PublisherParam {
				param_name,
				param_type,
			} => {
				let param_pattern = format!("{} : {}", param_name, param_type);
				let param_pattern_spaced =
					format!("{}: {}", param_name, param_type);
				let found = code.contains(&param_pattern)
					|| code.contains(&param_pattern_spaced);
				assert!(
					found,
					"Test '{}': missing publisher parameter '{}' with type \
					 '{}'\nGenerated code: {}",
					test_name, param_name, param_type, code
				);
			}
			| CodeCheck::FormatString(format_str) => {
				assert!(
					code.contains(format_str),
					"Test '{}': missing format string '{}'\nGenerated code: {}",
					test_name,
					format_str,
					code
				);
			}
			| CodeCheck::NotPresent(text) => {
				assert!(
					!code.contains(text),
					"Test '{}': found unexpected text '{}'\nGenerated code: {}",
					test_name,
					text,
					code
				);
			}
		}
	}
}

/// Run a comprehensive test case
fn run_codegen_test(test_case: CodegenTestCase) {
	let (generator, test_struct) = create_generator(
		test_case.struct_fields,
		test_case.pattern,
		test_case.generation_config,
	);

	let complete = generator
		.generate_complete_implementation(&test_struct)
		.unwrap_or_else(|_| {
			panic!(
				"Test '{}' should generate code successfully",
				test_case.name
			)
		});

	let code = complete.to_string();
	verify_generated_code(&code, test_case.expected_checks, test_case.name);
}

#[test]
fn test_generator_configuration_modes() {
	let test_cases = vec![
		(
			"subscriber_only",
			GenerationConfig::SubscriberOnly,
			true,
			false,
		),
		(
			"publisher_only",
			GenerationConfig::PublisherOnly,
			false,
			true,
		),
		("both_modes", GenerationConfig::Both, true, true),
	];

	for (name, config, expected_subscriber, expected_publisher) in test_cases {
		let (generator, _) = create_generator(
			quote! { sensor_id: u32, payload: String },
			"sensors/{sensor_id}/data",
			config,
		);

		assert_eq!(
			generator.should_generate_subscriber(),
			expected_subscriber,
			"Test '{}': subscriber generation mismatch",
			name
		);
		assert_eq!(
			generator.should_generate_publisher(),
			expected_publisher,
			"Test '{}': publisher generation mismatch",
			name
		);
	}
}

#[test]
fn test_subscriber_only_generation() {
	let test_case = CodegenTestCase {
		name: "subscriber_only",
		pattern: "sensors/{sensor_id}/data",
		struct_fields: quote! {
			sensor_id: u32,
			payload: String,
		},
		generation_config: GenerationConfig::SubscriberOnly,
		expected_checks: vec![
			CodeCheck::TraitImpl("FromMqttMessage"),
			CodeCheck::Method("subscribe"),
			CodeCheck::ParamExtraction {
				param_name: "sensor_id",
				index: 0,
			},
			CodeCheck::FieldAssignment("sensor_id"),
			CodeCheck::FieldAssignment("payload"),
			CodeCheck::Constant {
				name: "TOPIC_PATTERN",
				value: "sensors/{sensor_id}/data",
			},
			CodeCheck::Constant {
				name: "MQTT_PATTERN",
				value: "sensors/+/data",
			},
			// Should NOT have publisher methods
			CodeCheck::NotPresent("pub async fn publish"),
			CodeCheck::NotPresent("pub fn get_publisher"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_publisher_only_generation() {
	let test_case = CodegenTestCase {
		name: "publisher_only",
		pattern: "sensors/{sensor_id}/data",
		struct_fields: quote! {
			sensor_id: u32,
			payload: String,
		},
		generation_config: GenerationConfig::PublisherOnly,
		expected_checks: vec![
			// Should have constants
			CodeCheck::Constant {
				name: "TOPIC_PATTERN",
				value: "sensors/{sensor_id}/data",
			},
			CodeCheck::Constant {
				name: "MQTT_PATTERN",
				value: "sensors/+/data",
			},
			// Should have publisher methods
			CodeCheck::Method("publish"),
			CodeCheck::Method("get_publisher"),
			CodeCheck::PublisherParam {
				param_name: "sensor_id",
				param_type: "u32",
			},
			CodeCheck::FormatString("sensors/{}/data"),
			// Should NOT have subscriber methods
			CodeCheck::NotPresent("FromMqttMessage"),
			CodeCheck::NotPresent("pub async fn subscribe"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_both_modes_generation() {
	let test_case = CodegenTestCase {
		name: "both_modes",
		pattern: "sensors/{sensor_id}/{room}/data",
		struct_fields: quote! {
			sensor_id: u32,
			room: String,
			payload: f64,
			topic: Arc<TopicMatch>,
		},
		generation_config: GenerationConfig::Both,
		expected_checks: vec![
			// Subscriber functionality
			CodeCheck::TraitImpl("FromMqttMessage"),
			CodeCheck::Method("subscribe"),
			CodeCheck::ParamExtraction {
				param_name: "sensor_id",
				index: 0,
			},
			CodeCheck::ParamExtraction {
				param_name: "room",
				index: 1,
			},
			CodeCheck::FieldAssignment("sensor_id"),
			CodeCheck::FieldAssignment("room"),
			CodeCheck::FieldAssignment("payload"),
			CodeCheck::FieldAssignment("topic"),
			// Publisher functionality
			CodeCheck::Method("publish"),
			CodeCheck::Method("get_publisher"),
			CodeCheck::PublisherParam {
				param_name: "sensor_id",
				param_type: "u32",
			},
			CodeCheck::PublisherParam {
				param_name: "room",
				param_type: "String",
			},
			CodeCheck::FormatString("sensors/{}/{}/data"),
			// Constants
			CodeCheck::Constant {
				name: "TOPIC_PATTERN",
				value: "sensors/{sensor_id}/{room}/data",
			},
			CodeCheck::Constant {
				name: "MQTT_PATTERN",
				value: "sensors/+/+/data",
			},
			CodeCheck::PayloadType("f64"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_anonymous_wildcards_publisher() {
	let test_case = CodegenTestCase {
		name: "anonymous_wildcards",
		pattern: "devices/+/{device_id}/+/status",
		struct_fields: quote! {
			device_id: String,
			payload: bool,
		},
		generation_config: GenerationConfig::PublisherOnly,
		expected_checks: vec![
			CodeCheck::Method("publish"),
			CodeCheck::Method("get_publisher"),
			// Anonymous wildcard parameters
			CodeCheck::PublisherParam {
				param_name: "wildcard_0",
				param_type: "& str",
			},
			CodeCheck::PublisherParam {
				param_name: "device_id",
				param_type: "String",
			},
			CodeCheck::PublisherParam {
				param_name: "wildcard_2",
				param_type: "& str",
			},
			CodeCheck::FormatString("devices/{}/{}/{}/status"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_mixed_parameter_types() {
	// Test when some named parameters have struct fields, others don't
	let test_case = CodegenTestCase {
		name: "mixed_parameter_types",
		pattern: "api/{version}/{user_id}/{category}",
		struct_fields: quote! {
			version: String,
			// user_id missing - should use &str
			category: u32,
			payload: serde_json::Value,
		},
		generation_config: GenerationConfig::PublisherOnly,
		expected_checks: vec![
			CodeCheck::PublisherParam {
				param_name: "version",
				param_type: "String",
			},
			CodeCheck::PublisherParam {
				param_name: "user_id",
				param_type: "& str", // No struct field, defaults to &str
			},
			CodeCheck::PublisherParam {
				param_name: "category",
				param_type: "u32",
			},
			CodeCheck::FormatString("api/{}/{}/{}"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_complex_publisher_pattern() {
	let test_case = CodegenTestCase {
		name: "complex_publisher",
		pattern: "buildings/{building}/floors/{floor}/rooms/{room}/devices/\
		          {device_id}/data",
		struct_fields: quote! {
			building: String,
			floor: u32,
			room: String,
			device_id: uuid::Uuid,
			payload: Vec<u8>,
		},
		generation_config: GenerationConfig::PublisherOnly,
		expected_checks: vec![
			CodeCheck::PublisherParam {
				param_name: "building",
				param_type: "String",
			},
			CodeCheck::PublisherParam {
				param_name: "floor",
				param_type: "u32",
			},
			CodeCheck::PublisherParam {
				param_name: "room",
				param_type: "String",
			},
			CodeCheck::PublisherParam {
				param_name: "device_id",
				param_type: "uuid :: Uuid",
			},
			CodeCheck::FormatString(
				"buildings/{}/floors/{}/rooms/{}/devices/{}/data",
			),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_no_payload_field() {
	let test_case = CodegenTestCase {
		name: "no_payload_field",
		pattern: "heartbeat/{service}",
		struct_fields: quote! {
			service: String,
		},
		generation_config: GenerationConfig::Both,
		expected_checks: vec![
			CodeCheck::TraitImpl("FromMqttMessage"),
			CodeCheck::Method("subscribe"),
			CodeCheck::Method("publish"),
			CodeCheck::FieldAssignment("service"),
			CodeCheck::PublisherParam {
				param_name: "service",
				param_type: "String",
			},
			CodeCheck::PayloadType("Vec < u8 >"), // Default payload type
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_empty_struct_with_anonymous_wildcards() {
	let test_case = CodegenTestCase {
		name: "empty_struct_anonymous",
		pattern: "sensors/+/+/data",
		struct_fields: quote! {},
		generation_config: GenerationConfig::Both,
		expected_checks: vec![
			CodeCheck::TraitImpl("FromMqttMessage"),
			CodeCheck::Method("subscribe"),
			CodeCheck::Method("publish"),
			CodeCheck::PublisherParam {
				param_name: "wildcard_0",
				param_type: "& str",
			},
			CodeCheck::PublisherParam {
				param_name: "wildcard_1",
				param_type: "& str",
			},
			CodeCheck::FormatString("sensors/{}/{}/data"),
			CodeCheck::PayloadType("Vec < u8 >"), // Default payload
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_generator_info_methods() {
	let test_cases = vec![
		(
			"basic_struct",
			quote! {
				sensor_id: u32,
				payload: String,
			},
			"sensors/{sensor_id}/data",
			1,     // param_count
			true,  // has_payload
			false, // has_topic_field
			vec!["sensor_id"],
		),
		(
			"full_struct",
			quote! {
				sensor_id: u32,
				room: String,
				payload: Vec<u8>,
				topic: Arc<TopicMatch>,
			},
			"sensors/{sensor_id}/{room}/data",
			2,    // param_count
			true, // has_payload
			true, // has_topic_field
			vec!["sensor_id", "room"],
		),
		(
			"minimal_struct",
			quote! {
				service: String,
			},
			"heartbeat/{service}",
			1,     // param_count
			false, // has_payload
			false, // has_topic_field
			vec!["service"],
		),
	];

	for (
		name,
		fields,
		pattern,
		expected_count,
		expected_payload,
		expected_topic,
		expected_names,
	) in test_cases
	{
		let (generator, _) =
			create_generator(fields, pattern, GenerationConfig::Both);

		assert_eq!(
			generator.context.topic_params.len(),
			expected_count,
			"Test '{}': param count mismatch",
			name
		);
		assert_eq!(
			generator.context.payload_type.is_some(),
			expected_payload,
			"Test '{}': payload presence mismatch",
			name
		);
		assert_eq!(
			generator.context.has_topic_field, expected_topic,
			"Test '{}': topic field presence mismatch",
			name
		);

		let actual_names: Vec<&str> = generator
			.context
			.topic_params
			.iter()
			.filter_map(|p| p.name.as_ref())
			.map(|name| name.as_str())
			.collect();
		assert_eq!(
			actual_names.len(),
			expected_names.len(),
			"Test '{}': param names count mismatch",
			name
		);
		for expected_name in expected_names {
			assert!(
				actual_names.contains(&expected_name),
				"Test '{}': missing parameter '{}'",
				name,
				expected_name
			);
		}
	}
}

#[test]
fn test_custom_field_types_in_publisher() {
	// Test that custom field types are properly used in publisher parameters
	let (generator, _) = create_generator(
		quote! {
			version: semver::Version,
			user_id: u64,
			category: std::borrow::Cow<'static, str>,
			item_id: uuid::Uuid,
			payload: serde_json::Value,
		},
		"api/{version}/{user_id}/{category}/{item_id}",
		GenerationConfig::PublisherOnly,
	);

	// Verify parameter names and types
	let _expected_types = [
		("version", "semver :: Version"),
		("user_id", "u64"),
		("category", "std :: borrow :: Cow < 'static , str >"),
		("item_id", "uuid :: Uuid"),
	];

	// This is more of an integration test - we'd need to check the actual
	// generated code to verify the types are correct
	assert_eq!(generator.context.topic_params.len(), 4);
	let param_names: Vec<&str> = generator
		.context
		.topic_params
		.iter()
		.filter_map(|p| p.name.as_ref())
		.map(|name| name.as_str())
		.collect();
	assert_eq!(
		param_names,
		vec!["version", "user_id", "category", "item_id"]
	);
}

#[test]
fn test_format_string_generation() {
	struct FormatTestCase {
		name: &'static str,
		pattern: &'static str,
		expected_format: &'static str,
	}

	let test_cases = vec![
		FormatTestCase {
			name: "simple_pattern",
			pattern: "sensors/{id}/data",
			expected_format: "sensors/{}/data",
		},
		FormatTestCase {
			name: "multiple_params",
			pattern: "buildings/{building}/floors/{floor}/rooms/{room}",
			expected_format: "buildings/{}/floors/{}/rooms/{}",
		},
		FormatTestCase {
			name: "mixed_wildcards",
			pattern: "devices/+/{device_id}/+/status",
			expected_format: "devices/{}/{}/{}/status",
		},
		FormatTestCase {
			name: "no_params",
			pattern: "static/path/data",
			expected_format: "static/path/data",
		},
		FormatTestCase {
			name: "only_anonymous",
			pattern: "sensors/+/+/data",
			expected_format: "sensors/{}/{}/data",
		},
	];

	for test_case in test_cases {
		let (generator, test_struct) = create_generator(
			quote! { payload: String },
			test_case.pattern,
			GenerationConfig::PublisherOnly,
		);

		let generated = generator
			.generate_complete_implementation(&test_struct)
			.expect("Should generate successfully");

		let code = generated.to_string();
		assert!(
			code.contains(test_case.expected_format),
			"Test '{}': missing format string '{}'\nGenerated: {}",
			test_case.name,
			test_case.expected_format,
			code
		);
	}
}

#[test]
fn test_comprehensive_integration() {
	// Integration test mimicking the prepare_for_macro_publisher.rs example
	let test_case = CodegenTestCase {
		name: "sensor_reading_integration",
		pattern: "typed/{room}/{sensor_id}/some/{temp}",
		struct_fields: quote! {
			sensor_id: u32,
			room: String,
			temp: f32,
			payload: SensorData,
			topic: Arc<TopicMatch>,
		},
		generation_config: GenerationConfig::Both,
		expected_checks: vec![
			// Subscriber parts
			CodeCheck::TraitImpl("FromMqttMessage"),
			CodeCheck::Method("subscribe"),
			CodeCheck::ParamExtraction {
				param_name: "room",
				index: 0,
			},
			CodeCheck::ParamExtraction {
				param_name: "sensor_id",
				index: 1,
			},
			CodeCheck::ParamExtraction {
				param_name: "temp",
				index: 2,
			},
			CodeCheck::FieldAssignment("sensor_id"),
			CodeCheck::FieldAssignment("room"),
			CodeCheck::FieldAssignment("temp"),
			CodeCheck::FieldAssignment("payload"),
			CodeCheck::FieldAssignment("topic"),
			// Publisher parts
			CodeCheck::Method("publish"),
			CodeCheck::Method("get_publisher"),
			CodeCheck::PublisherParam {
				param_name: "sensor_id",
				param_type: "u32",
			},
			CodeCheck::PublisherParam {
				param_name: "room",
				param_type: "String",
			},
			CodeCheck::PublisherParam {
				param_name: "temp",
				param_type: "f32",
			},
			CodeCheck::FormatString("typed/{}/{}/some/{}"),
			// Constants
			CodeCheck::Constant {
				name: "TOPIC_PATTERN",
				value: "typed/{room}/{sensor_id}/some/{temp}",
			},
			CodeCheck::Constant {
				name: "MQTT_PATTERN",
				value: "typed/+/+/some/+",
			},
			CodeCheck::PayloadType("SensorData"),
		],
	};

	run_codegen_test(test_case);
}

#[test]
fn test_edge_cases() {
	let edge_cases = vec![
		CodegenTestCase {
			name: "only_topic_field",
			pattern: "status/+",
			struct_fields: quote! {
				topic: Arc<TopicMatch>,
			},
			generation_config: GenerationConfig::SubscriberOnly,
			expected_checks: vec![
				CodeCheck::FieldAssignment("topic"),
				CodeCheck::PayloadType("Vec < u8 >"),
				CodeCheck::TraitImpl("FromMqttMessage"),
			],
		},
		CodegenTestCase {
			name: "single_anonymous_wildcard",
			pattern: "heartbeat/+",
			struct_fields: quote! {
				payload: String,
			},
			generation_config: GenerationConfig::PublisherOnly,
			expected_checks: vec![
				CodeCheck::Method("publish"),
				CodeCheck::PublisherParam {
					param_name: "wildcard_0",
					param_type: "& str",
				},
				CodeCheck::FormatString("heartbeat/{}"),
			],
		},
	];

	for test_case in edge_cases {
		run_codegen_test(test_case);
	}
}
