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
				//Never panic, this should always have an identifier
				.expect("Named fields should have identifiers")
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

	#[test]
	fn test_extract_available_params() {
		let pattern = create_topic_pattern("sensors/{sensor_id}/+/{room}/data");
		let params = StructAnalysisContext::extract_available_params(&pattern);

		assert_eq!(params.len(), 2);
		assert_eq!(params[0].name, "sensor_id");
		assert_eq!(params[0].wildcard_index, 0);
		assert_eq!(params[1].name, "room");
		assert_eq!(params[1].wildcard_index, 2);
	}

	#[test]
	fn test_extract_available_params_mixed_wildcards() {
		let pattern = create_topic_pattern("home/+/{device_id}/status/#");
		let params = StructAnalysisContext::extract_available_params(&pattern);

		assert_eq!(params.len(), 1);
		assert_eq!(params[0].name, "device_id");
		assert_eq!(params[0].wildcard_index, 1);
	}

	#[test]
	fn test_is_arc_topic_match_type() {
		// Valid types
		let valid_type: syn::Type = parse_quote!(Arc<TopicMatch>);
		assert!(StructAnalysisContext::is_arc_topic_match_type(&valid_type));

		let valid_type_full: syn::Type = parse_quote!(
			std::sync::Arc<mqtt_typed_client::topic::topic_match::TopicMatch>
		);
		assert!(StructAnalysisContext::is_arc_topic_match_type(
			&valid_type_full
		));

		// Invalid types
		let invalid_type: syn::Type = parse_quote!(Arc<String>);
		assert!(!StructAnalysisContext::is_arc_topic_match_type(
			&invalid_type
		));

		let invalid_type2: syn::Type = parse_quote!(TopicMatch);
		assert!(!StructAnalysisContext::is_arc_topic_match_type(
			&invalid_type2
		));

		let invalid_type3: syn::Type = parse_quote!(Vec<TopicMatch>);
		assert!(!StructAnalysisContext::is_arc_topic_match_type(
			&invalid_type3
		));
	}

	#[test]
	fn test_successful_analysis() {
		let pattern = create_topic_pattern("sensors/{sensor_id}/data");
		let test_struct = create_test_struct(quote! {
			sensor_id: u32,
			payload: String,
		});

		let context =
			StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();

		assert_eq!(context.topic_params.len(), 1);
		assert_eq!(context.topic_params[0].name, "sensor_id");
		assert!(context.payload_type.is_some());
		assert!(!context.has_topic_field);
	}

	#[test]
	fn test_unknown_field_error() {
		let pattern = create_topic_pattern("sensors/{sensor_id}/data");
		let test_struct = create_test_struct(quote! {
			sensor_id: u32,
			unknown_field: String,
		});

		let result = StructAnalysisContext::analyze(&test_struct, &pattern);
		assert!(result.is_err());

		let error_msg = result.unwrap_err().to_string();
		assert!(error_msg.contains("Unknown fields: [unknown_field]"));
		assert!(error_msg.contains("sensor_id"));
	}

	#[test]
	fn test_invalid_topic_field_type() {
		let pattern = create_topic_pattern("sensors/+/data");
		let test_struct = create_test_struct(quote! {
			topic: String,  // Wrong type
		});

		let result = StructAnalysisContext::analyze(&test_struct, &pattern);
		assert!(result.is_err());

		let error_msg = result.unwrap_err().to_string();
		assert!(
			error_msg.contains("Field 'topic' must be of type Arc<TopicMatch>")
		);
	}

	#[test]
	fn test_non_struct_input() {
		let pattern = create_topic_pattern("test/+");
		let test_enum: syn::DeriveInput = parse_quote! {
			enum TestEnum {
				Variant1,
				Variant2,
			}
		};

		let result = StructAnalysisContext::analyze(&test_enum, &pattern);
		assert!(result.is_err());

		let error_msg = result.unwrap_err().to_string();
		assert!(error_msg.contains("can only be applied to structs with"));
	}

	#[test]
	fn test_tuple_struct() {
		let pattern = create_topic_pattern("test/+");
		let test_struct: syn::DeriveInput = parse_quote! {
			struct TestStruct(u32, String);
		};

		let result = StructAnalysisContext::analyze(&test_struct, &pattern);
		assert!(result.is_err());

		let error_msg = result.unwrap_err().to_string();
		assert!(error_msg.contains("named fields, not tuple structs"));
	}

	#[test]
	fn test_helper_methods() {
		let pattern = create_topic_pattern("sensors/{sensor_id}/{room}/data");
		let test_struct = create_test_struct(quote! {
			sensor_id: u32,
			room: String,
			payload: Vec<u8>,
			topic: Arc<TopicMatch>,
		});

		let context =
			StructAnalysisContext::analyze(&test_struct, &pattern).unwrap();

		assert_eq!(context.param_count(), 2);
		assert!(context.has_special_fields());

		let param_names = context.param_names();
		assert_eq!(param_names.len(), 2);
		assert!(param_names.contains(&"sensor_id"));
		assert!(param_names.contains(&"room"));
	}
}
