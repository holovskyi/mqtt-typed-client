//! Struct analysis and validation logic
//!
//! This module handles the analysis of user-defined structs and topic patterns,
//! validating that they are compatible and extracting all necessary information
//! for code generation.

use mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath;
use syn::{Data, DataStruct, Fields, Path, PathSegment, TypePath};

/// Represents a topic parameter with its name and position in the wildcard sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicParam {
	/// The parameter name (e.g., "sensor_id" from "{sensor_id}")
	pub name: String,
	/// Index among ALL wildcards in the pattern (used for extraction)
	pub wildcard_index: usize,
}

/// Contains all validated information about the struct and its relationship to the topic pattern
#[derive(Debug)]
pub struct StructAnalysisContext {
	/// Type of the payload field, if present
	pub payload_type: Option<syn::Type>,
	/// Whether the struct has a topic field of type Arc<TopicMatch>
	pub has_topic_field: bool,
	/// Topic parameters that have corresponding struct fields
	pub topic_params: Vec<TopicParam>,
}

impl StructAnalysisContext {
	/// Analyze the struct and topic pattern, returning a context for code generation
	/// # Validation Rules
	/// 1. Must be a struct with named fields
	/// 2. Only allowed fields: "payload", "topic", and topic parameter names
	/// 3. "topic" field must be of type `Arc<TopicMatch>` if present
	/// 4. Parameter fields must correspond to named wildcards in the pattern
	pub fn analyze(
		input_struct: &syn::DeriveInput,
		topic_pattern: &TopicPatternPath,
	) -> Result<Self, syn::Error> {
		let struct_fields = Self::extract_struct_fields(input_struct)?;
		let available_params = Self::extract_available_params(topic_pattern);

		let mut context = Self {
			payload_type: None,
			has_topic_field: false,
			topic_params: Vec::new(),
		};

		let mut unknown_fields = Vec::new();

		// Process each struct field and categorize it
		for field in struct_fields {
			let field_name = field
				.ident
				.as_ref()
				// Safe: we already validated this is a struct with named fields
				.unwrap()
				.to_string();

			match field_name.as_str() {
				| "payload" => {
					context.payload_type = Some(field.ty.clone());
				}

				| "topic" => {
					Self::validate_topic_field_type(&field.ty)?;
					context.has_topic_field = true;
				}

				| _ => {
					// Check if this field corresponds to a topic parameter
					if let Some(param) =
						available_params.iter().find(|p| p.name == field_name)
					{
						context.topic_params.push(param.clone());
					} else {
						unknown_fields.push(field_name);
					}
				}
			}
		}

		// Validate that all fields are recognized
		if !unknown_fields.is_empty() {
			let available_param_names: Vec<&str> =
				available_params.iter().map(|p| p.name.as_str()).collect();

			return Err(syn::Error::new_spanned(
				input_struct,
				format!(
					"Unknown fields: [{}]. Allowed fields: 'payload', \
					 'topic', and topic parameters: [{}]",
					unknown_fields.join(", "),
					available_param_names.join(", ")
				),
			));
		}

		Ok(context)
	}

	/// Extract named fields from struct with validation
	///
	/// Ensures the input is a struct with named fields, returning an error otherwise.
	fn extract_struct_fields(
		input_struct: &syn::DeriveInput,
	) -> Result<
		&syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
		syn::Error,
	> {
		match &input_struct.data {
			| Data::Struct(DataStruct {
				fields: Fields::Named(fields),
				..
			}) => Ok(&fields.named),
			| _ => Err(syn::Error::new_spanned(
				input_struct,
				"mqtt_topic_subscriber can only be applied to structs with \
				 named fields, not tuple structs or unit structs",
			)),
		}
	}

	/// Extract all available topic parameters with their wildcard indices
	///
	/// Scans the topic pattern for named wildcards (e.g., `{sensor_id}`, `{room:#}`)
	/// and assigns each one an index based on its position among ALL wildcards.
	///
	/// # Example
	/// For pattern "sensors/+/{sensor_id}/data/{room}":
	/// - Anonymous wildcard `+` at index 0 (not returned)
	/// - Named wildcard `{sensor_id}` at index 1 → TopicParam { name: "sensor_id", wildcard_index: 1 }
	/// - Named wildcard `{room}` at index 2 → TopicParam { name: "room", wildcard_index: 2 }
	fn extract_available_params(
		topic_pattern: &TopicPatternPath,
	) -> Vec<TopicParam> {
		topic_pattern
			.iter()
			.filter(|item| item.is_wildcard())
			.enumerate()
			.filter_map(|(i, item)| {
				item.param_name().map(|name| (i, name.to_string()))
			})
			.map(|(wildcard_index, name)| TopicParam {
				name,
				wildcard_index,
			})
			.collect()
	}

	/// Validate that a topic field has the correct type: `Arc<TopicMatch>`
	///
	/// Performs syntactic analysis of the type to ensure it matches exactly
	/// `Arc<TopicMatch>` or `std::sync::Arc<mqtt_typed_client::topic::topic_match::TopicMatch>`.
	fn validate_topic_field_type(ty: &syn::Type) -> Result<(), syn::Error> {
		if !Self::is_arc_topic_match_type(ty) {
			return Err(syn::Error::new_spanned(
				ty,
				"Field 'topic' must be of type Arc<TopicMatch>. Import it as: \
				 use std::sync::Arc; use \
				 mqtt_typed_client::topic::topic_match::TopicMatch;",
			));
		}
		Ok(())
	}

	/// Check if a type syntactically matches `Arc<TopicMatch>`
	///
	/// This is a best-effort check that looks for the Arc<...> pattern with
	/// TopicMatch as the inner type. It handles various import styles but
	/// may not catch all edge cases.
    fn is_arc_topic_match_type(ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Path(type_path) => {
                // Look for the last segment being "Arc"
                if let Some(arc_segment) = type_path.path.segments.last() {
                    if arc_segment.ident == "Arc" {
                        // Check if Arc has angle-bracketed generic arguments
                        if let syn::PathArguments::AngleBracketed(args) = &arc_segment.arguments {
                            // Look for the first generic argument being TopicMatch
                            if let Some(syn::GenericArgument::Type(syn::Type::Path(inner_path))) = args.args.first() {
                                // Check if the inner type ends with TopicMatch
                                if let Some(inner_segment) = inner_path.path.segments.last() {
                                    return inner_segment.ident == "TopicMatch";
                                }
                            }
                        }
                    }
                }
            }
            _ => return false,
        }
        false
    }

	/// Get the number of topic parameters that need to be extracted
	pub fn param_count(&self) -> usize {
		self.topic_params.len()
	}

	/// Check if the struct has any fields that need special handling
	pub fn has_special_fields(&self) -> bool {
		self.payload_type.is_some() || self.has_topic_field
	}

	/// Get all topic parameter names
	pub fn param_names(&self) -> Vec<&str> {
		self.topic_params.iter().map(|p| p.name.as_str()).collect()
	}
}

#[cfg(test)]
mod tests {
	use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
	use quote::quote;
	use syn::parse_quote;

	use super::*;

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
	fn create_topic_pattern(pattern: &str) -> TopicPatternPath {
		TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
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
			AnalysisResult::Success {
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
					context.has_topic_field,
					has_topic_field,
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
			AnalysisResult::Error { error_contains } => {
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
	fn test_extract_available_params() {
		struct ParamTestCase {
			pattern: &'static str,
			expected: Vec<(&'static str, usize)>, // (name, wildcard_index)
		}

		let test_cases = vec![
			ParamTestCase {
				pattern: "sensors/{sensor_id}/+/{room}/data",
				expected: vec![("sensor_id", 0), ("room", 2)],
			},
			ParamTestCase {
				pattern: "home/+/{device_id}/status/#",
				expected: vec![("device_id", 1)],
			},
			ParamTestCase {
				pattern: "simple/+/topic",
				expected: vec![],
			},
			ParamTestCase {
				pattern: "{a}/{b}/{c}",
				expected: vec![("a", 0), ("b", 1), ("c", 2)],
			},
		];

		for (i, test_case) in test_cases.into_iter().enumerate() {
			let pattern = create_topic_pattern(test_case.pattern);
			let params = StructAnalysisContext::extract_available_params(&pattern);

			assert_eq!(
				params.len(),
				test_case.expected.len(),
				"Test case {}: param count mismatch for pattern '{}'",
				i,
				test_case.pattern
			);

			for (param, (expected_name, expected_index)) in
				params.iter().zip(test_case.expected.iter())
			{
				assert_eq!(
					param.name,
					*expected_name,
					"Test case {}: param name mismatch",
					i
				);
				assert_eq!(
					param.wildcard_index,
					*expected_index,
					"Test case {}: wildcard index mismatch for '{}'",
					i,
					param.name
				);
			}
		}
	}

	#[test]
	fn test_is_arc_topic_match_type() {
		struct TypeTestCase {
			name: &'static str,
			type_tokens: proc_macro2::TokenStream,
			expected: bool,
		}

		let test_cases = vec![
			// Valid types
			TypeTestCase {
				name: "simple_arc_topic_match",
				type_tokens: quote!(Arc<TopicMatch>),
				expected: true,
			},
			TypeTestCase {
				name: "fully_qualified_arc_topic_match",
				type_tokens: quote!(
					std::sync::Arc<mqtt_typed_client::topic::topic_match::TopicMatch>
				),
				expected: true,
			},
			// Invalid types
			TypeTestCase {
				name: "arc_with_wrong_inner_type",
				type_tokens: quote!(Arc<String>),
				expected: false,
			},
			TypeTestCase {
				name: "topic_match_without_arc",
				type_tokens: quote!(TopicMatch),
				expected: false,
			},
			TypeTestCase {
				name: "vec_with_topic_match",
				type_tokens: quote!(Vec<TopicMatch>),
				expected: false,
			},
			TypeTestCase {
				name: "box_with_topic_match",
				type_tokens: quote!(Box<TopicMatch>),
				expected: false,
			},
			TypeTestCase {
				name: "primitive_type",
				type_tokens: quote!(u32),
				expected: false,
			},
		];

		for test_case in test_cases {
			let parsed_type: syn::Type = syn::parse2(test_case.type_tokens)
				.expect(&format!("Failed to parse type for test '{}'", test_case.name));

			let result = StructAnalysisContext::is_arc_topic_match_type(&parsed_type);
			assert_eq!(
				result,
				test_case.expected,
				"Test '{}': expected {}, got {}",
				test_case.name,
				test_case.expected,
				result
			);
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
					param_count: 0,
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
		assert!(result
			.unwrap_err()
			.to_string()
			.contains("can only be applied to structs"));

		// Test tuple struct
		let test_tuple: syn::DeriveInput = parse_quote! {
			struct TestStruct(u32, String);
		};
		let result = StructAnalysisContext::analyze(&test_tuple, &pattern);
		assert!(result.is_err());
		assert!(result
			.unwrap_err()
			.to_string()
			.contains("named fields"));
	}
}
