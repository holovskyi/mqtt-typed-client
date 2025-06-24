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
		let (from_mqtt_impl, subscriber_methods) =
			if self.should_generate_subscriber() {
				let from_mqtt_impl =
					self.generate_from_mqtt_impl(&input_struct.ident)?;
				let subscriber_methods = self.generate_helper_methods();
				(from_mqtt_impl, subscriber_methods)
			} else {
				(quote! {}, quote! {})
			};
		let publisher_methods = if self.should_generate_publisher() {
			self.generate_publisher_methods()?
		} else {
			quote! {}
		};
		let constants = self.generate_constants();
		let struct_name = &input_struct.ident;
		Ok(quote! {
			#input_struct
			#from_mqtt_impl
			impl #struct_name {
				#constants
				#subscriber_methods
				#publisher_methods
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

	/// Generate topic pattern constants
	fn generate_constants(&self) -> proc_macro2::TokenStream {
		let topic_pattern = &self.macro_args.pattern;
		let topic_pattern_literal = topic_pattern.topic_pattern().to_string();
		let mqtt_pattern_literal = topic_pattern.mqtt_pattern().to_string();
		quote! {
				/// The original topic pattern with named parameters (e.g., "sensors/{sensor_id}/data")
				pub const TOPIC_PATTERN: &'static str = #topic_pattern_literal;

				/// The MQTT subscription pattern with wildcards (e.g., "sensors/+/data")
				pub const MQTT_PATTERN: &'static str = #mqtt_pattern_literal;
		}
	}
	/// Generate helper methods and constants for the struct
	///
	/// Creates:
	/// - `TOPIC_PATTERN`: Original pattern with named parameters
	/// - `MQTT_PATTERN`: MQTT subscription pattern with wildcards
	/// - `subscribe()`: Async method to create a typed subscriber
	fn generate_helper_methods(&self) -> proc_macro2::TokenStream {
		let payload_type = self.get_payload_type_token();

		quote! {

				/// Subscribe to this topic pattern using the provided MQTT client
				///
				/// # Arguments
				/// * `client` - The MQTT client to use for subscription
				///
				/// # Returns
				/// A structured subscriber that yields instances of this struct
				///
				/// # Example
				/// ```rust,no_run
				/// # use mqtt_typed_client::MqttClient;
				/// # use mqtt_typed_client_macros::mqtt_topic_subscriber;
				/// # #[derive(Debug)]
				/// # #[mqtt_topic_subscriber("sensors/{sensor_id}/data")]
				/// # struct SensorReading { sensor_id: u32, payload: String }
				/// # async fn example(client: &MqttClient<mqtt_typed_client::BincodeSerializer>) {
				/// let mut subscriber = SensorReading::subscribe(client).await?;
				/// while let Some(result) = subscriber.receive().await {
				///     match result {
				///         Ok(reading) => println!("Sensor {}: {}", reading.sensor_id, reading.payload),
				///         Err(e) => eprintln!("Error: {}", e),
				///     }
				/// }
				/// # }
				/// ```
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
					let subscriber = client.subscribe::<#payload_type>(Self::MQTT_PATTERN).await?;
					Ok(::mqtt_typed_client::MqttTopicSubscriber::new(subscriber))
				}
		}
	}

	/// Generate publisher methods: publish() and get_publisher()
	///
	/// Creates methods for publishing messages to the topic pattern with
	/// extracted parameters as function arguments.
	fn generate_publisher_methods(
		&self,
	) -> Result<proc_macro2::TokenStream, syn::Error> {
		let payload_type = self.get_payload_type_token();
		let method_params = self.get_publisher_method_params();
		let (format_string, format_args) = self.get_topic_format_and_args();

		// Generate parameter documentation
		let param_docs: Vec<proc_macro2::TokenStream> = self
			.context
			.topic_params
			.iter()
			.map(|param| {
				let param_name = param.get_publisher_param_name();
				let doc_text = if param.is_anonymous() {
					format!(
						"/// * `{}` - Value for anonymous wildcard",
						param_name
					)
				} else {
					format!(
						"/// * `{}` - Value for the {{{}}} parameter",
						param_name,
						param.name.as_ref().unwrap()
					)
				};
				quote! { #[doc = #doc_text] }
			})
			.collect();

		Ok(quote! {
			/// Publish a message to this topic pattern
			///
			/// # Arguments
			/// * `client` - The MQTT client to use for publishing
			#(#param_docs)*
			/// * `data` - The message payload to publish
			///
			/// # Returns
			/// Result indicating success or failure of the publish operation
			pub async fn publish<F>(
				client: &::mqtt_typed_client::MqttClient<F>,
				#(#method_params,)*
				data: &#payload_type,
			) -> ::std::result::Result<(), ::mqtt_typed_client::MqttClientError>
			where
				F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
			{
				let publisher = Self::get_publisher(client #(, #format_args)*)?;
				publisher.publish(data).await
			}

			/// Get a publisher for this topic pattern
			///
			/// # Arguments
			/// * `client` - The MQTT client to use for publishing
			#(#param_docs)*
			///
			/// # Returns
			/// A topic publisher that can be used to publish messages
			pub fn get_publisher<F>(
				client: &::mqtt_typed_client::MqttClient<F>,
				#(#method_params,)*
			) -> ::std::result::Result<
				::mqtt_typed_client::MqttPublisher<#payload_type, F>,
				::mqtt_typed_client::errors::TopicRouterError,
			>
			where
				F: ::mqtt_typed_client::MessageSerializer<#payload_type>,
			{
				let topic = format!(#format_string #(, #format_args)*);
				client.get_publisher::<#payload_type>(&topic)
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

	/// Get format string and arguments for topic construction
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
