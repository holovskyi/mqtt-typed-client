//! Code generation logic
//!
//! This module handles the generation of Rust code based on the analyzed struct
//! and topic pattern information. It generates trait implementations and helper
//! methods for MQTT topic subscribers.

use quote::{format_ident, quote};

use crate::{
	MacroArgs,
	analysis::{StructAnalysisContext, TopicParam},
};

/// Handles all code generation for MQTT topic subscribers
///
/// Takes validated analysis context and generates the necessary Rust code
/// including trait implementations and helper methods.
pub struct CodeGenerator {
	pub context: StructAnalysisContext,
	macro_args: MacroArgs,
}

impl CodeGenerator {
	/// Create a new code generator with the given analysis context
	pub fn new(context: StructAnalysisContext, macro_args: MacroArgs) -> Self {
		Self {
			context,
			macro_args,
		}
	}

	/// Check if subscriber code should be generated
	pub fn should_generate_subscriber(&self) -> bool {
		self.macro_args.generate_subscriber
	}

	/// Check if publisher code should be generated
	pub fn should_generate_publisher(&self) -> bool {
		self.macro_args.generate_publisher
	}

	/// Check if typed client code should be generated
	pub fn should_generate_typed_client(&self) -> bool {
		self.macro_args.generate_typed_client
	}

	/// Check if last will code should be generated
	pub fn should_generate_last_will(&self) -> bool {
		self.macro_args.generate_last_will
	}

	/// Check if prelude code should be generated
	pub fn should_generate_prelude(&self) -> bool {
		self.macro_args.generate_prelude
	}

	/// Generate complete implementation including original struct, traits, and helper methods
	///
	/// # Arguments
	/// * `input_struct` - The original struct definition to preserve
	/// * `topic_pattern` - The topic pattern for generating constants and methods
	///
	/// # Returns
	/// Complete token stream ready for macro expansion
	pub fn generate_complete_implementation(
		&self,
		input_struct: &syn::DeriveInput,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let struct_name = &input_struct.ident;

		let (from_mqtt_impl, subscriber_methods, subscription_filter_extension) =
			if self.should_generate_subscriber() {
				let from_mqtt_impl =
					self.generate_from_mqtt_impl(struct_name)?;
				let subscriber_methods = self.generate_helper_methods();
				let subscription_filter_extension =
					self.generate_subscription_builder_extension(struct_name);
				(
					from_mqtt_impl,
					subscriber_methods,
					subscription_filter_extension,
				)
			} else {
				(quote! {}, quote! {}, quote! {})
			};
		let publisher_methods = if self.should_generate_publisher() {
			self.generate_publisher_methods()?
		} else {
			quote! {}
		};

		let last_will_methods = if self.should_generate_last_will() {
			self.generate_last_will_methods()
		} else {
			quote! {}
		};

		let typed_client_extension = if self.should_generate_typed_client() {
			let generator =
				crate::codegen_typed_client::TypedClientGenerator::new(
					self,
					struct_name,
				);
			generator.generate_complete_typed_client()
		} else {
			quote! {}
		};

		let prelude_module = self.generate_prelude_module(struct_name);

		let constants = self.generate_constants();
		let builder_methods = Self::generate_builder_methods();

		Ok(quote! {
			#input_struct
			#from_mqtt_impl
			#typed_client_extension
			#subscription_filter_extension
			#prelude_module

			impl #struct_name {
				#constants
				#builder_methods
				#subscriber_methods
				#publisher_methods
				#last_will_methods
			}
		})
	}

	/// Generate the `FromMqttMessage` trait implementation
	///
	/// Creates an implementation that can convert from MQTT topic and payload
	/// into the user's struct, extracting topic parameters and handling
	/// payload deserialization.
	fn generate_from_mqtt_impl(
		&self,
		struct_name: &syn::Ident,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let param_extractions = self.generate_param_extractions();
		let field_assignments = self.generate_field_assignments();
		let payload_type = self.get_payload_type_token();

		Ok(quote! {
			impl<DE> ::mqtt_typed_client_core::FromMqttMessage<#payload_type, DE> for #struct_name {
				fn from_mqtt_message(
					topic: ::std::sync::Arc<::mqtt_typed_client_core::topic::topic_match::TopicMatch>,
					payload: #payload_type,
				) -> ::std::result::Result<Self, ::mqtt_typed_client_core::MessageConversionError<DE>> {
					#(#param_extractions)*

					Ok(Self {
						#(#field_assignments)*
					})
				}
			}
		})
	}

	/// Generate default pattern and subscription builder methods
	fn generate_builder_methods() -> proc_macro2::TokenStream {
		quote! {
			/// Get default topic pattern for this message type
			pub fn default_pattern() -> &'static ::mqtt_typed_client_core::TopicPatternPath {
				use std::sync::OnceLock;
				static PATTERN: OnceLock<::mqtt_typed_client_core::TopicPatternPath> = OnceLock::new();
				PATTERN.get_or_init(|| {
					::mqtt_typed_client_core::TopicPatternPath::new_from_string(
						Self::TOPIC_PATTERN,
						::mqtt_typed_client_core::CacheStrategy::NoCache
					).expect("Built-in pattern must be valid")
				})
			}

			/// Create subscription builder with default configuration
			pub fn subscription<F>(client: &::mqtt_typed_client_core::MqttClient<F>,
			) -> ::mqtt_typed_client_core::SubscriptionBuilder<Self,F>
			where
				F: Clone
			{
				::mqtt_typed_client_core::SubscriptionBuilder::new(client.clone(),
					Self::default_pattern().clone()
				)
			}
		}
	}

	/// Generate topic pattern constants
	fn generate_constants(&self) -> proc_macro2::TokenStream {
		let topic_pattern = &self.macro_args.pattern;
		let topic_pattern_literal = topic_pattern.topic_pattern().to_string();
		let mqtt_pattern_literal = topic_pattern.mqtt_pattern().to_string();
		quote! {
				pub const TOPIC_PATTERN: &'static str = #topic_pattern_literal;
				pub const MQTT_PATTERN: &'static str = #mqtt_pattern_literal;
		}
	}
	/// Generate helper methods for subscription
	fn generate_helper_methods(&self) -> proc_macro2::TokenStream {
		let payload_type = self.get_payload_type_token();

		quote! {
			/// Subscribe with default configuration
			pub async fn subscribe<F>(
				client: &::mqtt_typed_client_core::MqttClient<F>,
			) -> ::std::result::Result<
				::mqtt_typed_client_core::MqttTopicSubscriber<Self, #payload_type, F>,
				::mqtt_typed_client_core::MqttClientError,
			>
			where
				F: ::std::default::Default
					+ ::std::clone::Clone
					+ ::std::marker::Send
					+ ::std::marker::Sync
					+ ::mqtt_typed_client_core::MessageSerializer<#payload_type>,
			{
				Self::subscription(client).subscribe().await
			}
		}
	}

	/// Generate publisher methods
	fn generate_publisher_methods(
		&self,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let payload_type = self.get_payload_type_token();
		let method_params = self.get_publisher_method_params();
		let (format_string, format_args) = self.get_topic_format_and_args();

		// Suppress clippy::ptr_arg for generated methods that may take &Vec<T> or &String
		// parameters. These warnings are not actionable in macro-generated code since
		// the parameter types are derived from user struct fields.
		Ok(quote! {
			/// Publish message to default topic
			#[allow(clippy::ptr_arg)]
			pub async fn publish<F>(
				client: &::mqtt_typed_client_core::MqttClient<F>,
				#(#method_params,)*
				data: &#payload_type,
			) -> ::std::result::Result<(), ::mqtt_typed_client_core::MqttClientError>
			where
				F: ::mqtt_typed_client_core::MessageSerializer<#payload_type>,
			{
				Self::get_publisher(client #(, #format_args)*)?.publish(data).await
			}

			/// Get publisher for default topic
			pub fn get_publisher<F>(
				client: &::mqtt_typed_client_core::MqttClient<F>,
				#(#method_params,)*
			) -> ::std::result::Result<
				::mqtt_typed_client_core::MqttPublisher<#payload_type, F>,
				::mqtt_typed_client_core::TopicError,
			>
			where
				F: ::mqtt_typed_client_core::MessageSerializer<#payload_type>,
			{
				let topic = format!(#format_string #(, #format_args)*);
				client.get_publisher::<#payload_type>(&topic)
			}

			pub fn get_publisher_to<F>(
				client: &::mqtt_typed_client_core::MqttClient<F>,
				custom_pattern: impl TryInto <
					::mqtt_typed_client_core::TopicPatternPath,
					Error = ::mqtt_typed_client_core::TopicPatternError,
				>,
				#(#method_params,)*
			) -> ::std::result::Result<
				::mqtt_typed_client_core::MqttPublisher<#payload_type, F>,
				::mqtt_typed_client_core::TopicError,
			>
			where
				F: ::mqtt_typed_client_core::MessageSerializer<#payload_type>,
			{
				let custom_pattern = custom_pattern.try_into()?;
				let default_pattern = Self::default_pattern();

				let validated_pattern = default_pattern
					.check_pattern_compatibility(custom_pattern)?;

				let topic =
					validated_pattern.format_topic(&[#(&#format_args as &dyn ::std::fmt::Display),*])?;

				client.get_publisher::<#payload_type>(&topic)
			}

		})
	}

	/// Generate last will methods
	pub fn generate_last_will_methods(&self) -> proc_macro2::TokenStream {
		let payload_type = self.get_payload_type_token();
		let method_params = self.get_publisher_method_params();
		let (format_string, format_args) = self.get_topic_format_and_args();

		quote! {
			/// Create Last Will message for default topic pattern
			pub fn last_will(
				#(#method_params,)*
				payload: #payload_type,
			) -> ::mqtt_typed_client_core::TypedLastWill<#payload_type> {
				let topic = format!(#format_string #(, #format_args)*);
				::mqtt_typed_client_core::TypedLastWill::new(topic, payload)
			}

			/// Create Last Will message for custom topic pattern
			pub fn last_will_to(
				custom_pattern: impl TryInto <
					::mqtt_typed_client_core::TopicPatternPath,
					Error = ::mqtt_typed_client_core::TopicPatternError,
				>,
				#(#method_params,)*
				payload: #payload_type,
			) -> ::std::result::Result <
				::mqtt_typed_client_core::TypedLastWill<#payload_type>,
				::mqtt_typed_client_core::TopicError,
			> {
				let custom_pattern = custom_pattern.try_into()?;
				let default_pattern = Self::default_pattern();

				let validated_pattern = default_pattern
					.check_pattern_compatibility(custom_pattern)?;

				let topic = validated_pattern
					.format_topic(&[#(&#format_args as &dyn ::std::fmt::Display),*])?;

				Ok(::mqtt_typed_client_core::TypedLastWill::new(topic, payload))
			}
		}
	}

	/// Generate extension trait with default implementations
	pub fn generate_subscription_builder_extension(
		&self,
		struct_name: &syn::Ident,
	) -> proc_macro2::TokenStream {
		let trait_name = format_ident!("{}SubscriptionBuilderExt", struct_name);
		let (filter_defs, filter_methods): (Vec<_>, Vec<_>) =
			self.generate_for_methods().into_iter().unzip();

		quote! {
			/// Extension trait for filtering subscription builder parameters
			pub trait #trait_name<F> {
				#(#filter_defs)*
			}

			impl<F: Clone> #trait_name<F> for ::mqtt_typed_client_core::SubscriptionBuilder<#struct_name, F> {
				#(#filter_methods)*
			}
		}
	}

	/// Generate filter methods with full implementations
	fn generate_for_methods(
		&self,
	) -> Vec<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
		self.context
			.topic_params
			.iter()
			.map(|param| self.generate_single_for_method(param))
			.collect()
	}

	/// Generate complete filter method with body
	fn generate_single_for_method(
		&self,
		param: &TopicParam,
	) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
		let method_name =
			format_ident!("for_{}", param.get_publisher_param_name());
		let param_type = param.get_publisher_param_type();
		let param_key = param.get_publisher_param_name();

		let declaration = quote! {
			fn #method_name(self, value: #param_type) -> Self;
		};
		let body = quote! {
			fn #method_name(self, value: #param_type) -> Self
			{
				self.bind_parameter(#param_key, value.to_string()).unwrap()
			}
		};
		(declaration, body)
	}

	/// Generate prelude module with all generated types
	pub fn generate_prelude_module(
		&self,
		struct_name: &syn::Ident,
	) -> proc_macro2::TokenStream {
		if !self.should_generate_prelude() {
			return quote! {};
		}
		let names =
			crate::naming::TypedClientNames::from_struct_name(struct_name);

		let mut exports = Vec::new();

		// Export main struct (with allow unused)
		exports.push(quote! {
			#[allow(unused_imports)]
			pub use super::#struct_name;
		});

		// Export payload type if it exists
		if let Some(payload_type) = &self.context.payload_type {
			exports.push(quote! {
				pub use super::#payload_type;
			});
		}

		// Export subscriber types if generated
		if self.should_generate_subscriber() {
			let subscription_builder_trait =
				format_ident!("{}SubscriptionBuilderExt", struct_name);
			exports.push(quote! {
				pub use super::#subscription_builder_trait;
			});
		}

		// Export typed client types if generated
		if self.should_generate_typed_client() {
			let client_struct = &names.client_struct;
			let extension_trait = &names.extension_trait;
			exports.push(quote! {
				#[allow(unused_imports)]
				pub use super::#client_struct;
			});
			exports.push(quote! {
				pub use super::#extension_trait;
			});
		}

		let prelude_module = 
			format_ident!("{}", names.method_name);

		quote! {
			pub mod #prelude_module {
				#(#exports)*
			}
		}
	}

	/// Generate code to extract topic parameters from the matched topic
	///
	/// For each topic parameter in the struct, generates a call to
	/// `extract_topic_parameter` with the correct wildcard index.
	///
	/// # Example output
	/// ```rust,ignore
	/// let sensor_id = ::mqtt_typed_client_core::extract_topic_parameter(
	///     &topic,
	///     0,
	///     "sensor_id"
	/// )?;
	/// let room = ::mqtt_typed_client_core::extract_topic_parameter(
	///     &topic,
	///     1,
	///     "room"
	/// )?;
	/// ```
	fn generate_param_extractions(&self) -> Vec<proc_macro2::TokenStream> {
		self.context
			.topic_params
			.iter()
			.filter(|param| !param.is_anonymous())
			.map(|param| self.generate_single_param_extraction(param))
			.collect()
	}

	/// Generate code to extract a single topic parameter
	fn generate_single_param_extraction(
		&self,
		param: &TopicParam,
	) -> proc_macro2::TokenStream {
		let param_ident = format_ident!("{}", param.name.as_ref().unwrap());
		let param_index = param.wildcard_index;
		let param_name = &param.name;

		quote! {
			let #param_ident = ::mqtt_typed_client_core::extract_topic_parameter(
				&topic,
				#param_index,
				#param_name
			)?;
		}
	}

	/// Generate field assignments for the struct constructor
	///
	/// Creates assignments for all fields: topic parameters, payload, and topic.
	/// The order matches the order fields appear in the struct.
	///
	/// # Example output
	/// ```rust,ignore
	/// Ok(Self {
	///     sensor_id,
	///     room,
	///     payload,
	///     topic,
	/// })
	/// ```
	fn generate_field_assignments(&self) -> Vec<proc_macro2::TokenStream> {
		let mut assignments = Vec::new();

		// Add topic parameter fields
		self.context
			.topic_params
			.iter()
			.filter(|param| !param.is_anonymous())
			.for_each(|param| {
				let param_ident =
					format_ident!("{}", param.name.as_ref().unwrap());
				assignments.push(quote! { #param_ident, });
			});

		// Add payload field if present
		if self.context.payload_type.is_some() {
			assignments.push(quote! { payload, });
		}

		// Add topic field if present
		if self.context.has_topic_field {
			assignments.push(quote! { topic, });
		}

		assignments
	}

	/// Get parameters for publisher methods
	pub fn get_publisher_method_params(&self) -> Vec<proc_macro2::TokenStream> {
		self.context
			.topic_params
			.iter()
			.map(|param| {
				let param_name = param.get_publisher_param_name();
				let param_type = param.get_publisher_param_type();
				let param_ident = format_ident!("{}", param_name);
				quote! { #param_ident: #param_type }
			})
			.collect()
	}

	/// Get format arguments for topic string construction
	pub fn get_topic_format_and_args(
		&self,
	) -> (String, Vec<proc_macro2::TokenStream>) {
		let mut format_parts = Vec::new();
		let mut param_args = Vec::new();
		let mut param_index = 0;

		for item in self.macro_args.pattern.iter() {
			if item.is_wildcard() {
				format_parts.push("{}");
				if let Some(param) = self.context.topic_params.get(param_index)
				{
					let param_name = param.get_publisher_param_name();
					let param_ident = format_ident!("{}", param_name);
					param_args.push(quote! { #param_ident });
					param_index += 1;
				}
			} else {
				format_parts.push(item.as_str());
			}
		}

		let format_string = format_parts.join("/");
		(format_string, param_args)
	}

	/// Get the payload type token, defaulting to `Vec<u8>` if no payload field
	///
	/// This handles the case where a struct doesn't have a payload field
	/// but still needs to work with the trait system.
	pub fn get_payload_type_token(&self) -> proc_macro2::TokenStream {
		self.context
			.payload_type
			.as_ref()
			.map(|ty| quote! { #ty })
			.unwrap_or_else(|| quote! { Vec<u8> })
	}
}
