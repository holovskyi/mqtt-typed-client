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
	/// Parameter name for named wildcards, None for anonymous (+)
	pub name: Option<String>,
	/// Index among ALL wildcards in the pattern
	pub wildcard_index: usize,
	/// Type from struct field, None means use &str
	pub field_type: Option<syn::Type>,
}

impl TopicParam {
	/// Get the parameter name for publisher methods
	pub fn get_publisher_param_name(&self) -> String {
		match &self.name {
			| Some(name) => name.clone(),
			| None => format!("wildcard_{}", self.wildcard_index + 1),
		}
	}

	/// Get the parameter type for publisher methods  
	pub fn get_publisher_param_type(&self) -> syn::Type {
		self.field_type
			.clone()
			.unwrap_or_else(|| syn::parse_quote! { &str })
	}

	/// Is this an anonymous wildcard?
	pub fn is_anonymous(&self) -> bool {
		self.name.is_none()
	}

	/// Is this a named parameter that has a corresponding struct field?
	pub fn has_struct_field(&self) -> bool {
		self.field_type.is_some()
	}

	pub fn build_topic_params(
		topic_pattern: &TopicPatternPath,
		field_types: &std::collections::HashMap<String, syn::Type>,
	) -> Vec<TopicParam> {
		topic_pattern
			.iter()
			.filter(|item| item.is_wildcard())
			.enumerate()
			.map(|(wildcard_index, item)| {
				let name = item.param_name().map(|s| s.to_string());
				let field_type = name
					.as_ref()
					.and_then(|param_name| field_types.get(param_name))
					.cloned();

				TopicParam {
					name,
					wildcard_index,
					field_type,
				}
			})
			.collect()
	}
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
	pub fn analyze(
		input_struct: &syn::DeriveInput,
		topic_pattern: &TopicPatternPath,
	) -> Result<Self, syn::Error> {
		let struct_fields = Self::extract_struct_fields(input_struct)?;
		let field_types = Self::extract_field_types(struct_fields)?;

		let mut unknown_fields = Vec::new();

		let mut payload_type = None;
		let mut has_topic_field = false;
		// Process each struct field and categorize it
		for field in struct_fields {
			let field_name = field.ident.as_ref().unwrap().to_string();

			match field_name.as_str() {
				| "payload" => {
					payload_type = Some(field.ty.clone());
				}
				| "topic" => {
					Self::validate_topic_field_type(&field.ty)?;
					has_topic_field = true;
				}
				| _ => {
					// Check if this field corresponds to a topic parameter
					let is_topic_param = topic_pattern
						.iter()
						.filter(|item| item.is_wildcard())
						.filter_map(|item| item.param_name())
						.any(|param_name| param_name == field_name);

					if !is_topic_param {
						unknown_fields.push(field_name);
					}
				}
			}
		}

		// Validate unknown fields
		if !unknown_fields.is_empty() {
			let named_params: Vec<_> = topic_pattern
				.iter()
				.filter(|item| item.is_wildcard())
				.filter_map(|item| item.param_name())
				.collect();

			return Err(syn::Error::new_spanned(
				input_struct,
				format!(
					"Unknown fields: [{}]. Allowed fields: 'payload', \
					 'topic', and topic parameters: [{}]",
					unknown_fields.join(", "),
					named_params.join(", ")
				),
			));
		}

		// Build topic parameters with field types
		let topic_params =
			TopicParam::build_topic_params(topic_pattern, &field_types);

		Ok(Self {
			payload_type,
			has_topic_field,
			topic_params,
		})
	}

	fn extract_field_types(
		fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
	) -> Result<std::collections::HashMap<String, syn::Type>, syn::Error> {
		let mut field_types = std::collections::HashMap::new();

		for field in fields {
			if let Some(ident) = &field.ident {
				let field_name = ident.to_string();
				if field_name != "payload" && field_name != "topic" {
					field_types.insert(field_name, field.ty.clone());
				}
			}
		}

		Ok(field_types)
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
				"mqtt_topic can only be applied to structs with \
				 named fields, not tuple structs or unit structs",
			)),
		}
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
			| syn::Type::Path(type_path) => {
				// Look for the last segment being "Arc"
				if let Some(arc_segment) = type_path.path.segments.last() {
					if arc_segment.ident == "Arc" {
						// Check if Arc has angle-bracketed generic arguments
						if let syn::PathArguments::AngleBracketed(args) =
							&arc_segment.arguments
						{
							// Look for the first generic argument being TopicMatch
							if let Some(syn::GenericArgument::Type(
								syn::Type::Path(inner_path),
							)) = args.args.first()
							{
								// Check if the inner type ends with TopicMatch
								if let Some(inner_segment) =
									inner_path.path.segments.last()
								{
									return inner_segment.ident == "TopicMatch";
								}
							}
						}
					}
				}
			}
			| _ => return false,
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
		self.topic_params.iter()
		.filter_map(|p| {
			p.name.as_ref().map(|name| name.as_str())
		})
		.collect()
	}
}
