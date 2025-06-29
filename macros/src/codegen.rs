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
		let (from_mqtt_impl, subscriber_methods) = if self
			.should_generate_subscriber()
		{
			let from_mqtt_impl = self.generate_from_mqtt_impl(struct_name)?;
			let subscriber_methods = self.generate_helper_methods();
			(from_mqtt_impl, subscriber_methods)
		} else {
			(quote! {}, quote! {})
		};
		let (publisher_methods, publisher_constructor, publisher_builder_impl) =
			if self.should_generate_publisher() {
				let publisher_methods =
					self.generate_publisher_methods(struct_name)?;
				let publisher_constructor =
					self.generate_publisher_constructor();
				let publisher_builder_impl =
					self.generate_publisher_builder_impl(struct_name)?;
				(
					publisher_methods,
					publisher_constructor,
					publisher_builder_impl,
				)
			} else {
				(quote! {}, quote! {}, quote! {})
			};

		let constants = self.generate_constants();
		let builder_methods = Self::generate_builder_methods();

		Ok(quote! {
			#input_struct
			#from_mqtt_impl
			impl #struct_name {
				#constants
				#builder_methods
				#publisher_constructor
				#subscriber_methods
				#publisher_methods
			}
			#publisher_builder_impl
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
			impl<DE> ::mqtt_typed_client::FromMqttMessage<#payload_type, DE> for #struct_name {
				fn from_mqtt_message(
					topic: ::std::sync::Arc<::mqtt_typed_client::topic::topic_match::TopicMatch>,
					payload: #payload_type,
				) -> ::std::result::Result<Self, ::mqtt_typed_client::MessageConversionError<DE>> {
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
			pub fn default_pattern() -> &'static ::mqtt_typed_client::TopicPatternPath {
				use std::sync::OnceLock;
				static PATTERN: OnceLock<::mqtt_typed_client::TopicPatternPath> = OnceLock::new();
				PATTERN.get_or_init(|| {
					::mqtt_typed_client::TopicPatternPath::new_from_string(
						Self::TOPIC_PATTERN,
						::mqtt_typed_client::CacheStrategy::NoCache
					).expect("Built-in pattern must be valid")
				})
			}

			/// Create subscription builder with default configuration
			pub fn subscription() -> ::mqtt_typed_client::SubscriptionBuilder<Self> {
				::mqtt_typed_client::SubscriptionBuilder::new(
					Self::default_pattern().clone()
				)
			}
		}
	}

	/// Generate publisher builder constructor
	fn generate_publisher_constructor(&self) -> proc_macro2::TokenStream {
		quote! {
			/// Create publisher builder with default configuration
			pub fn publisher() -> ::mqtt_typed_client::PublisherBuilder<Self> {
				::mqtt_typed_client::PublisherBuilder::new(
					Self::default_pattern().clone()
				)
			}
		}
	}

	/// Generate extension trait for PublisherBuilder
	fn generate_publisher_builder_impl(
		&self,
		struct_name: &syn::Ident,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let payload_type = self.get_payload_type_token();
		let method_params = self.get_publisher_method_params();
		let (format_string, format_args) = self.get_topic_format_and_args();
		let trait_name = format_ident!("{}PublisherExt", struct_name);

		// Suppress clippy::ptr_arg for generated methods that may take &Vec<T> or &String
		// parameters. These warnings are not actionable in macro-generated code since
		// the parameter types are derived from user struct fields.
		Ok(quote! {
			/// Extension trait for PublisherBuilder with type-specific methods
			#[allow(clippy::ptr_arg)]
			pub trait #trait_name {
				/// Publish message using builder configuration
				async fn publish<F>(
					&self,
					client: &::mqtt_typed_client::MqttClient<F>,
					#(#method_params,)*
					data: &#payload_type,
				) -> ::std::result::Result<(), ::mqtt_typed_client::MqttClientError>
				where
					F: ::mqtt_typed_client::MessageSerializer<#payload_type>;

				/// Get configured publisher
				fn get_publisher<F>(
					&self,
					client: &::mqtt_typed_client::MqttClient<F>,
					#(#method_params,)*
				) -> ::std::result::Result<
					::mqtt_typed_client::MqttPublisher<#payload_type, F>,
					::mqtt_typed_client::MqttClientError,
				>
				where
					F: ::mqtt_typed_client::MessageSerializer<#payload_type>;
			}

			impl #trait_name for ::mqtt_typed_client::PublisherBuilder<#struct_name> {
				#[allow(clippy::ptr_arg)]
				async fn publish<F>(
					&self,
					client: &::mqtt_typed_client::MqttClient<F>,
					#(#method_params,)*
					data: &#payload_type,
				) -> ::std::result::Result<(), ::mqtt_typed_client::MqttClientError>
				where
					F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
				{
					let publisher = self.get_publisher(client #(, #format_args)*)?;
					publisher.publish(data).await
				}

				fn get_publisher<F>(
					&self,
					client: &::mqtt_typed_client::MqttClient<F>,
					#(#method_params,)*
				) -> ::std::result::Result<
					::mqtt_typed_client::MqttPublisher<#payload_type, F>,
					::mqtt_typed_client::MqttClientError,
				>
				where
					F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
				{
					// for default pattern, we can use format! directly for performance
					let topic = if self.is_pattern_overridden() {
							self.pattern()
								.format_topic(&[#(&#format_args as &dyn ::std::fmt::Display),*])
								.map_err(|e| ::mqtt_typed_client::MqttClientError::Topic(e.into()))?
						} else {
							format!(#format_string #(, #format_args)*)
						};
					Ok(client.get_publisher::<#payload_type>(&topic)?
						.with_qos(self.qos())
						.with_retain(self.retain()))
				}
			}
		})
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
				client: &::mqtt_typed_client::MqttClient<F>,
			) -> ::std::result::Result<
				::mqtt_typed_client::MqttTopicSubscriber<Self, #payload_type, F>,
				::mqtt_typed_client::MqttClientError,
			>
			where
				F: ::std::default::Default
					+ ::std::clone::Clone
					+ ::std::marker::Send
					+ ::std::marker::Sync
					+ ::mqtt_typed_client::MessageSerializer<#payload_type>,
			{
				Self::subscription().subscribe(client).await
			}
		}
	}

	/// Generate publisher methods
	fn generate_publisher_methods(
		&self,
		struct_name: &syn::Ident,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let payload_type = self.get_payload_type_token();
		let method_params = self.get_publisher_method_params();
		let param_args: Vec<_> = self
			.context
			.topic_params
			.iter()
			.map(|param| {
				let param_name = param.get_publisher_param_name();
				let param_ident = format_ident!("{}", param_name);
				quote! { #param_ident }
			})
			.collect();
		let trait_name = format_ident!("{}PublisherExt", struct_name);

		// Suppress clippy::ptr_arg for generated methods that may take &Vec<T> or &String
		// parameters. These warnings are not actionable in macro-generated code since
		// the parameter types are derived from user struct fields.
		Ok(quote! {
			/// Publish message to this topic
			#[allow(clippy::ptr_arg)]
			pub async fn publish<F>(
				client: &::mqtt_typed_client::MqttClient<F>,
				#(#method_params,)*
				data: &#payload_type,
			) -> ::std::result::Result<(), ::mqtt_typed_client::MqttClientError>
			where
				F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
			{
				use #trait_name;
				Self::publisher().publish(client #(, #param_args)*, data).await
			}

			/// Get publisher for this topic
			pub fn get_publisher<F>(
				client: &::mqtt_typed_client::MqttClient<F>,
				#(#method_params,)*
			) -> ::std::result::Result<
				::mqtt_typed_client::MqttPublisher<#payload_type, F>,
				::mqtt_typed_client::MqttClientError,
			>
			where
				F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
			{
				use #trait_name;
				Self::publisher().get_publisher(client #(, #param_args)*)
			}
		})
	}

	/// Generate code to extract topic parameters from the matched topic
	///
	/// For each topic parameter in the struct, generates a call to
	/// `extract_topic_parameter` with the correct wildcard index.
	///
	/// # Example output
	/// ```rust,ignore
	/// let sensor_id = ::mqtt_typed_client::extract_topic_parameter(
	///     &topic,
	///     0,
	///     "sensor_id"
	/// )?;
	/// let room = ::mqtt_typed_client::extract_topic_parameter(
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
			let #param_ident = ::mqtt_typed_client::extract_topic_parameter(
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
	fn get_publisher_method_params(&self) -> Vec<proc_macro2::TokenStream> {
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
	fn get_topic_format_and_args(
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
	fn get_payload_type_token(&self) -> proc_macro2::TokenStream {
		self.context
			.payload_type
			.as_ref()
			.map(|ty| quote! { #ty })
			.unwrap_or_else(|| quote! { Vec<u8> })
	}
}
