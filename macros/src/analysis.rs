//! Struct analysis and validation logic
//!
//! This module handles the analysis of user-defined structs and topic patterns,
//! validating that they are compatible and extracting all necessary information
//! for code generation.

use mqtt_typed_client_core::topic::topic_pattern_path::TopicPatternPath;
use syn::{Data, DataStruct, Fields};

/// Field names the macro fills automatically. They are never treated as topic
/// parameters, and using one as a *named* wildcard while also declaring it as a
/// field is a hard error (see `reject_reserved_wildcard`).
pub(crate) const RESERVED_FIELD_NAMES: [&str; 3] = ["payload", "topic", "meta"];

/// Represents a topic parameter with its name and position in the wildcard sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicParam {
	/// Parameter name for named wildcards, None for anonymous (+)
	pub name: Option<String>,
	/// Index among ALL wildcards in the pattern
	pub wildcard_index: usize,
	/// Type from struct field
	pub struct_field_type: Option<syn::Type>,
}

impl TopicParam {
	/// Get the parameter name for publisher methods
	pub fn get_publisher_param_name(&self) -> String {
		match &self.name {
			| Some(name) => name.clone(),
			| None => format!("wildcard_{}", self.wildcard_index),
		}
	}

	/// Get the parameter type for publisher methods  
	pub fn get_publisher_param_type(&self) -> syn::Type {
		match &self.struct_field_type {
			| Some(field_type) => {
				if Self::is_string_type(field_type) {
					syn::parse_quote! { &str }
				} else {
					field_type.clone()
				}
			}
			| None => syn::parse_quote! { &str },
		}
	}

	pub fn is_struct_field(&self) -> bool {
		self.struct_field_type.is_some()
	}

	fn is_string_type(ty: &syn::Type) -> bool {
		matches!(ty, syn::Type::Path(path)
		  if path.path.segments.last().map(|s| s.ident == "String").unwrap_or(false))
	}

	#[cfg(test)]
	/// Is this an anonymous wildcard?
	pub fn is_anonymous(&self) -> bool {
		self.name.is_none()
	}

	/// Build topic parameters from pattern and struct field types
	///
	/// Maps wildcard positions in the topic pattern to corresponding struct fields,
	/// preserving wildcard order and associating correct types.
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
					struct_field_type: field_type,
				}
			})
			.collect()
	}
}

#[cfg(test)]
/// Test utilities for StructAnalysisContext
impl StructAnalysisContext {
	/// Create a context from components
	///
	/// Useful for testing and advanced usage where you need to construct
	/// the analysis context manually.
	pub fn from_components(
		payload_type: Option<syn::Type>,
		has_topic_field: bool,
		topic_params: Vec<TopicParam>,
	) -> Self {
		Self {
			payload_type,
			has_topic_field,
			topic_field_owned: false,
			has_meta_field: false,
			meta_field_owned: false,
			topic_params,
		}
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
		self.topic_params
			.iter()
			.filter_map(|p| p.name.as_deref())
			.collect()
	}
}

/// Contains all validated information about the struct and its relationship to the topic pattern
#[derive(Debug)]
pub struct StructAnalysisContext {
	/// Type of the payload field, if present
	pub payload_type: Option<syn::Type>,
	/// Whether the struct has a `topic` field (`TopicMatch` or `Arc<TopicMatch>`)
	pub has_topic_field: bool,
	/// When `has_topic_field`: the field was declared as the bare `TopicMatch`
	/// (owned — codegen emits `Arc::unwrap_or_clone`) rather than
	/// `Arc<TopicMatch>` (shared — the `Arc` is moved in as-is).
	pub topic_field_owned: bool,
	/// Whether the struct has a `meta` field (`MessageMeta` or `Arc<MessageMeta>`)
	pub has_meta_field: bool,
	/// As `topic_field_owned`, for the `meta` field.
	pub meta_field_owned: bool,
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
		let mut topic_field_owned = false;
		let mut has_meta_field = false;
		let mut meta_field_owned = false;
		// Process each struct field and categorize it
		for field in struct_fields {
			let field_name = field.ident.as_ref().unwrap().to_string();

			match field_name.as_str() {
				| "payload" => {
					Self::reject_reserved_wildcard(
						topic_pattern,
						"payload",
						input_struct,
					)?;
					payload_type = Some(field.ty.clone());
				}
				| "topic" => {
					Self::reject_reserved_wildcard(
						topic_pattern,
						"topic",
						input_struct,
					)?;
					topic_field_owned = Self::validate_adaptive_field(
						&field.ty,
						"TopicMatch",
						"topic",
					)?;
					has_topic_field = true;
				}
				| "meta" => {
					Self::reject_reserved_wildcard(
						topic_pattern,
						"meta",
						input_struct,
					)?;
					meta_field_owned = Self::validate_adaptive_field(
						&field.ty,
						"MessageMeta",
						"meta",
					)?;
					has_meta_field = true;
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
					 'topic', 'meta', and topic parameters: [{}]",
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
			topic_field_owned,
			has_meta_field,
			meta_field_owned,
			topic_params,
		})
	}

	/// Extract field name to type mappings, excluding the reserved fields the
	/// macro fills itself (`payload`, `topic`, `meta`).
	fn extract_field_types(
		fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
	) -> Result<std::collections::HashMap<String, syn::Type>, syn::Error> {
		let mut field_types = std::collections::HashMap::new();

		for field in fields {
			if let Some(ident) = &field.ident {
				let field_name = ident.to_string();
				if !RESERVED_FIELD_NAMES.contains(&field_name.as_str()) {
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
				"mqtt_topic can only be applied to structs with named fields, \
				 not tuple structs or unit structs",
			)),
		}
	}

	/// Reject a reserved field name that is *also* used as a named wildcard in
	/// the pattern (the silent-misbind bug: the field would steal the payload /
	/// topic slot while the wildcard loses its value). Narrow by design — a
	/// `{topic}` wildcard with no `topic` field is a perfectly good param, so
	/// this only fires from a reserved-field match arm, where the field exists.
	fn reject_reserved_wildcard(
		topic_pattern: &TopicPatternPath,
		name: &str,
		span_src: &syn::DeriveInput,
	) -> Result<(), syn::Error> {
		let collides = topic_pattern
			.iter()
			.filter(|item| item.is_wildcard())
			.filter_map(|item| item.param_name())
			.any(|param_name| param_name == name);

		if collides {
			return Err(syn::Error::new_spanned(
				span_src,
				format!(
					"topic pattern uses `{{{name}}}` as a named wildcard, but \
					 `{name}` is a reserved field name that the macro fills \
					 automatically (payload, topic, meta). Rename the \
					 wildcard, e.g. `{{{name}_id}}`."
				),
			));
		}
		Ok(())
	}

	/// Validate an Arc-adaptive reserved field (`topic`/`meta`): it must be the
	/// bare `inner` type or `Arc<inner>`. Returns `true` when it is owned (bare),
	/// so codegen can emit `Arc::unwrap_or_clone`; `false` for the shared `Arc`.
	fn validate_adaptive_field(
		ty: &syn::Type,
		inner: &str,
		field: &str,
	) -> Result<bool, syn::Error> {
		if Self::is_arc_of(ty, inner) {
			Ok(false)
		} else if Self::is_plain_type(ty, inner) {
			Ok(true)
		} else {
			Err(syn::Error::new_spanned(
				ty,
				format!(
					"Field '{field}' must be of type `{inner}` or \
					 `Arc<{inner}>`."
				),
			))
		}
	}

	/// Check if a type syntactically matches `Arc<inner>` (any import style for
	/// `Arc`, inner matched by its last path segment).
	fn is_arc_of(ty: &syn::Type, inner: &str) -> bool {
		let syn::Type::Path(type_path) = ty else {
			return false;
		};
		let Some(arc_segment) = type_path.path.segments.last() else {
			return false;
		};
		if arc_segment.ident != "Arc" {
			return false;
		}
		let syn::PathArguments::AngleBracketed(args) = &arc_segment.arguments
		else {
			return false;
		};
		matches!(
			args.args.first(),
			Some(syn::GenericArgument::Type(syn::Type::Path(inner_path)))
				if inner_path.path.segments.last()
					.is_some_and(|s| s.ident == inner)
		)
	}

	/// Check if a type is the bare `name` (matched by its last path segment).
	fn is_plain_type(ty: &syn::Type, name: &str) -> bool {
		matches!(ty, syn::Type::Path(p)
			if p.path.segments.last().is_some_and(|s| s.ident == name))
	}
}
