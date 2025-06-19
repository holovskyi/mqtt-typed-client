//! Code generation logic
//!
//! This module handles the generation of Rust code based on the analyzed struct
//! and topic pattern information. It generates trait implementations and helper
//! methods for MQTT topic subscribers.

use crate::analysis::{StructAnalysisContext, TopicParam};
use mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath;
use quote::{format_ident, quote};

/// Handles all code generation for MQTT topic subscribers
///
/// Takes validated analysis context and generates the necessary Rust code
/// including trait implementations and helper methods.
pub struct CodeGenerator {
    context: StructAnalysisContext,
}

impl CodeGenerator {
    /// Create a new code generator with the given analysis context
    pub fn new(context: StructAnalysisContext) -> Self {
        Self { context }
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
        topic_pattern: &TopicPatternPath,
    ) -> Result<proc_macro2::TokenStream, syn::Error> {
        let original_struct = input_struct;
        let from_mqtt_impl = self.generate_from_mqtt_impl(&input_struct.ident)?;
        let helper_methods = self.generate_helper_methods(&input_struct.ident, topic_pattern);

        Ok(quote! {
            #original_struct
            #from_mqtt_impl
            #helper_methods
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

    /// Generate helper methods and constants for the struct
    ///
    /// Creates:
    /// - `TOPIC_PATTERN`: Original pattern with named parameters
    /// - `MQTT_PATTERN`: MQTT subscription pattern with wildcards
    /// - `subscribe()`: Async method to create a typed subscriber
    fn generate_helper_methods(
        &self,
        struct_name: &syn::Ident,
        topic_pattern: &TopicPatternPath,
    ) -> proc_macro2::TokenStream {
        let payload_type = self.get_payload_type_token();
        let topic_pattern_literal = topic_pattern.topic_pattern().to_string();
        let mqtt_pattern_literal = topic_pattern.mqtt_pattern().to_string();

        quote! {
            impl #struct_name {
                /// The original topic pattern with named parameters (e.g., "sensors/{sensor_id}/data")
                pub const TOPIC_PATTERN: &'static str = #topic_pattern_literal;

                /// The MQTT subscription pattern with wildcards (e.g., "sensors/+/data")
                pub const MQTT_PATTERN: &'static str = #mqtt_pattern_literal;

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
                    ::mqtt_typed_client::MqttStructuredSubscriber<Self, #payload_type, F>,
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
                    Ok(::mqtt_typed_client::MqttStructuredSubscriber::new(subscriber))
                }
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
            .map(|param| self.generate_single_param_extraction(param))
            .collect()
    }

    /// Generate code to extract a single topic parameter
    fn generate_single_param_extraction(&self, param: &TopicParam) -> proc_macro2::TokenStream {
        let param_ident = format_ident!("{}", param.name);
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
        for param in &self.context.topic_params {
            let param_ident = format_ident!("{}", param.name);
            assignments.push(quote! { #param_ident, });
        }

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

    /// Get the number of topic parameters that will be extracted
    pub fn param_count(&self) -> usize {
        self.context.topic_params.len()
    }

    /// Check if the struct has a payload field
    pub fn has_payload(&self) -> bool {
        self.context.payload_type.is_some()
    }

    /// Check if the struct has a topic field
    pub fn has_topic_field(&self) -> bool {
        self.context.has_topic_field
    }

    /// Get the names of all topic parameters
    pub fn param_names(&self) -> Vec<&str> {
        self.context.topic_params.iter().map(|p| p.name.as_str()).collect()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::StructAnalysisContext;
    use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
    use quote::quote;
    use syn::parse_quote;

    /// Helper to create a topic pattern for testing
    fn create_topic_pattern(pattern: &str) -> TopicPatternPath {
        TopicPatternPath::new_from_string(pattern, CacheStrategy::NoCache)
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

    /// Helper to analyze and create a code generator
    fn create_generator(
        struct_fields: proc_macro2::TokenStream,
        pattern: &str,
    ) -> (CodeGenerator, syn::DeriveInput, TopicPatternPath) {
        let test_struct = create_test_struct(struct_fields);
        let topic_pattern = create_topic_pattern(pattern);
        let context = StructAnalysisContext::analyze(&test_struct, &topic_pattern)
            .expect("Analysis should succeed");
        let generator = CodeGenerator::new(context);
        
        (generator, test_struct, topic_pattern)
    }

    #[test]
    fn test_generate_param_extractions() {
        let (generator, _, _) = create_generator(
            quote! {
                sensor_id: u32,
                room: String,
                payload: Vec<u8>,
            },
            "sensors/{sensor_id}/temperature/{room}",
        );

        let extractions = generator.generate_param_extractions();
        assert_eq!(extractions.len(), 2);
        
        // Check that the generated code contains the expected parameter names
        let code = quote! { #(#extractions)* }.to_string();
        assert!(code.contains("sensor_id"));
        assert!(code.contains("room"));
        assert!(code.contains("extract_topic_parameter"));
    }

    #[test]
    fn test_generate_field_assignments() {
        let (generator, _, _) = create_generator(
            quote! {
                sensor_id: u32,
                payload: String,
                topic: Arc<TopicMatch>,
            },
            "sensors/{sensor_id}/data",
        );

        let assignments = generator.generate_field_assignments();
        assert_eq!(assignments.len(), 3); // sensor_id + payload + topic
        
        let code = quote! { #(#assignments)* }.to_string();
        assert!(code.contains("sensor_id"));
        assert!(code.contains("payload"));
        assert!(code.contains("topic"));
    }

    #[test]
    fn test_get_payload_type_token_with_custom_type() {
        let (generator, _, _) = create_generator(
            quote! {
                sensor_id: u32,
                payload: String,
            },
            "sensors/{sensor_id}/data",
        );

        let payload_type = generator.get_payload_type_token();
        assert_eq!(payload_type.to_string(), "String");
    }

    #[test]
    fn test_get_payload_type_token_default() {
        let (generator, _, _) = create_generator(
            quote! {
                sensor_id: u32,
            },
            "sensors/{sensor_id}/data",
        );

        let payload_type = generator.get_payload_type_token();
        assert_eq!(payload_type.to_string(), "Vec < u8 >");
    }

    #[test]
    fn test_generate_helper_methods() {
        let (generator, test_struct, topic_pattern) = create_generator(
            quote! {
                sensor_id: u32,
                payload: String,
            },
            "sensors/{sensor_id}/data",
        );

        let methods = generator.generate_helper_methods(&test_struct.ident, &topic_pattern);
        let code = methods.to_string();
        
        // Check for constants
        assert!(code.contains("TOPIC_PATTERN"));
        assert!(code.contains("MQTT_PATTERN"));
        assert!(code.contains("sensors/{sensor_id}/data"));
        assert!(code.contains("sensors/+/data"));
        
        // Check for subscribe method
        assert!(code.contains("pub async fn subscribe"));
        assert!(code.contains("MqttStructuredSubscriber"));
    }

    #[test]
    fn test_generate_from_mqtt_impl() {
        let (generator, test_struct, _) = create_generator(
            quote! {
                sensor_id: u32,
                payload: String,
            },
            "sensors/{sensor_id}/data",
        );

        let impl_code = generator.generate_from_mqtt_impl(&test_struct.ident).unwrap();
        let code = impl_code.to_string();
        
        // Check trait implementation
        assert!(code.contains("impl < DE > :: mqtt_typed_client :: FromMqttMessage"));
        assert!(code.contains("fn from_mqtt_message"));
        assert!(code.contains("extract_topic_parameter"));
        assert!(code.contains("Ok (Self {"));
    }

    #[test]
    fn test_generate_complete_implementation() {
        let (generator, test_struct, topic_pattern) = create_generator(
            quote! {
                sensor_id: u32,
                payload: String,
            },
            "sensors/{sensor_id}/data",
        );

        let complete = generator
            .generate_complete_implementation(&test_struct, &topic_pattern)
            .unwrap();
        let code = complete.to_string();
        
        // Should contain original struct
        assert!(code.contains("struct TestStruct"));
        
        // Should contain trait implementation
        assert!(code.contains("impl < DE >"));
        assert!(code.contains("FromMqttMessage"));
        
        // Should contain helper methods
        assert!(code.contains("TOPIC_PATTERN"));
        assert!(code.contains("subscribe"));
    }

    #[test]
    fn test_generator_info_methods() {
        let (generator, _, _) = create_generator(
            quote! {
                sensor_id: u32,
                room: String,
                payload: Vec<u8>,
                topic: Arc<TopicMatch>,
            },
            "sensors/{sensor_id}/{room}/data",
        );

        assert_eq!(generator.param_count(), 2);
        assert!(generator.has_payload());
        assert!(generator.has_topic_field());
        assert_eq!(generator.param_names(), vec!["sensor_id", "room"]);
    }

    #[test]
    fn test_minimal_struct() {
        let (generator, test_struct, topic_pattern) = create_generator(
            quote! {},  // Empty struct
            "sensors/+/data",
        );

        let complete = generator
            .generate_complete_implementation(&test_struct, &topic_pattern)
            .unwrap();
        
        // Should still generate valid code even for empty struct
        let code = complete.to_string();
        assert!(code.contains("struct TestStruct"));
        assert!(code.contains("FromMqttMessage"));
        assert!(code.contains("Vec < u8 >")); // Default payload type
    }

    #[test]
    fn test_complex_pattern() {
        let (generator, test_struct, topic_pattern) = create_generator(
            quote! {
                building: String,
                floor: u32,
                room: String,
                device_id: String,
                payload: serde_json::Value,
            },
            "buildings/{building}/floors/{floor}/rooms/{room}/devices/{device_id}/data",
        );

        let complete = generator
            .generate_complete_implementation(&test_struct, &topic_pattern)
            .unwrap();
        
        let code = complete.to_string();
        
        // Should handle multiple parameters correctly
        assert!(code.contains("building"));
        assert!(code.contains("floor"));
        assert!(code.contains("room"));
        assert!(code.contains("device_id"));
        assert!(code.contains("serde_json :: Value")); // Custom payload type
        
        assert_eq!(generator.param_count(), 4);
    }
}