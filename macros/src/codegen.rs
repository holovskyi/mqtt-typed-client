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

    /// Test case for code generation
    struct CodegenTestCase {
        name: &'static str,
        pattern: &'static str,
        struct_fields: proc_macro2::TokenStream,
        expected_checks: Vec<CodeCheck>,
    }

    /// What to check in generated code
    #[derive(Debug, Clone)]
    enum CodeCheck {
        /// Check that a parameter is extracted with correct index
        ParamExtraction { param_name: &'static str, index: usize },
        /// Check that field assignment exists
        FieldAssignment(&'static str),
        /// Check that constant is defined with correct value
        Constant { name: &'static str, value: &'static str },
        /// Check that method exists
        Method(&'static str),
        /// Check trait implementation
        TraitImpl(&'static str),
        /// Check payload type in generated code
        PayloadType(&'static str),
    }

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

    /// Run checks against generated code
    fn verify_generated_code(code: &str, checks: Vec<CodeCheck>, test_name: &str) {
        
        for check in checks {
            match check {
                CodeCheck::ParamExtraction { param_name, index } => {
                    // More flexible pattern matching for parameter extraction
                    // Check if the call exists with correct parameters, ignore spacing
                    let has_extract_call = code.contains("extract_topic_parameter");
                    let has_param_name = code.contains(&format!("\"{}\"", param_name));
                    let has_index_plain = code.contains(&format!("& topic , {} ,", index)) ||
                                         code.contains(&format!("&topic, {}, ", index)) ||
                                         code.contains(&format!("& topic , {}usize ,", index)) ||
                                         code.contains(&format!("&topic, {}usize,", index));
                    
                    let found = has_extract_call && has_param_name && has_index_plain;
                    assert!(
                        found,
                        "Test '{}': missing parameter extraction for '{}' at index {}\nExtract call: {}, Param name: {}, Index: {}\nGenerated code: {}",
                        test_name, param_name, index, has_extract_call, has_param_name, has_index_plain, code
                    );
                }
                CodeCheck::FieldAssignment(field) => {
                    let patterns = vec![
                        format!("{} ,", field),
                        format!("{},", field),
                    ];
                    let found = patterns.iter().any(|pattern| code.contains(pattern));
                    assert!(
                        found,
                        "Test '{}': missing field assignment for '{}'\nGenerated code: {}",
                        test_name, field, code
                    );
                }
                CodeCheck::Constant { name, value } => {
                    let patterns = vec![
                        format!("pub const {} : & 'static str = \"{}\" ;", name, value),
                        format!("pub const {}: &'static str = \"{}\";", name, value),
                        format!("pub const {} : &'static str = \"{}\" ;", name, value),
                    ];
                    let found = patterns.iter().any(|pattern| code.contains(pattern));
                    assert!(
                        found,
                        "Test '{}': missing constant '{}' with value '{}'\nGenerated code: {}",
                        test_name, name, value, code
                    );
                }
                CodeCheck::Method(method_name) => {
                    let patterns = vec![
                        format!("pub async fn {}", method_name),
                        format!("pub async fn {}(", method_name),
                    ];
                    let found = patterns.iter().any(|pattern| code.contains(pattern));
                    assert!(
                        found,
                        "Test '{}': missing method '{}'\nGenerated code: {}",
                        test_name, method_name, code
                    );
                }
                CodeCheck::TraitImpl(trait_name) => {
                    assert!(
                        code.contains(trait_name),
                        "Test '{}': missing trait implementation for '{}'\nGenerated code: {}",
                        test_name, trait_name, code
                    );
                }
                CodeCheck::PayloadType(type_name) => {
                    assert!(
                        code.contains(type_name),
                        "Test '{}': missing payload type '{}'\nGenerated code: {}",
                        test_name, type_name, code
                    );
                }
            }
        }
    }

    /// Run a comprehensive test case
    fn run_codegen_test(test_case: CodegenTestCase) {
        let (generator, test_struct, topic_pattern) = create_generator(
            test_case.struct_fields,
            test_case.pattern,
        );

        let complete = generator
            .generate_complete_implementation(&test_struct, &topic_pattern)
            .expect(&format!("Test '{}' should generate code successfully", test_case.name));
        
        let code = complete.to_string();
        verify_generated_code(&code, test_case.expected_checks, test_case.name);
    }

    #[test]
    fn test_comprehensive_code_generation() {
        let test_cases = vec![
            CodegenTestCase {
                name: "basic_sensor_reading",
                pattern: "sensors/{sensor_id}/data",
                struct_fields: quote! {
                    sensor_id: u32,
                    payload: String,
                },
                expected_checks: vec![
                    CodeCheck::ParamExtraction { param_name: "sensor_id", index: 0 },
                    CodeCheck::FieldAssignment("sensor_id"),
                    CodeCheck::FieldAssignment("payload"),
                    CodeCheck::Constant { name: "TOPIC_PATTERN", value: "sensors/{sensor_id}/data" },
                    CodeCheck::Constant { name: "MQTT_PATTERN", value: "sensors/+/data" },
                    CodeCheck::Method("subscribe"),
                    CodeCheck::TraitImpl("FromMqttMessage"),
                    CodeCheck::PayloadType("String"),
                ],
            },
            CodegenTestCase {
                name: "multi_param_with_topic",
                pattern: "sensors/{sensor_id}/{room}/temperature",
                struct_fields: quote! {
                    sensor_id: u32,
                    room: String,
                    payload: f64,
                    topic: Arc<TopicMatch>,
                },
                expected_checks: vec![
                    CodeCheck::ParamExtraction { param_name: "sensor_id", index: 0 },
                    CodeCheck::ParamExtraction { param_name: "room", index: 1 },
                    CodeCheck::FieldAssignment("sensor_id"),
                    CodeCheck::FieldAssignment("room"),
                    CodeCheck::FieldAssignment("payload"),
                    CodeCheck::FieldAssignment("topic"),
                    CodeCheck::PayloadType("f64"),
                ],
            },
            CodegenTestCase {
                name: "no_payload_field",
                pattern: "heartbeat/{service}",
                struct_fields: quote! {
                    service: String,
                },
                expected_checks: vec![
                    CodeCheck::ParamExtraction { param_name: "service", index: 0 },
                    CodeCheck::FieldAssignment("service"),
                    CodeCheck::PayloadType("Vec < u8 >"), // Default payload type
                ],
            },
            CodegenTestCase {
                name: "complex_pattern",
                pattern: "buildings/{building}/floors/{floor}/rooms/{room}/devices/{device_id}/data",
                struct_fields: quote! {
                    building: String,
                    floor: u32,
                    room: String,
                    device_id: String,
                    payload: serde_json::Value,
                },
                expected_checks: vec![
                    CodeCheck::ParamExtraction { param_name: "building", index: 0 },
                    CodeCheck::ParamExtraction { param_name: "floor", index: 1 },
                    CodeCheck::ParamExtraction { param_name: "room", index: 2 },
                    CodeCheck::ParamExtraction { param_name: "device_id", index: 3 },
                    CodeCheck::PayloadType("serde_json :: Value"),
                ],
            },
        ];

        for test_case in test_cases {
            run_codegen_test(test_case);
        }
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
                1, // param_count
                true, // has_payload
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
                2, // param_count
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
                1, // param_count
                false, // has_payload
                false, // has_topic_field
                vec!["service"],
            ),
        ];

        for (name, fields, pattern, expected_count, expected_payload, expected_topic, expected_names) in test_cases {
            let (generator, _, _) = create_generator(fields, pattern);

            assert_eq!(
                generator.param_count(),
                expected_count,
                "Test '{}': param count mismatch",
                name
            );
            assert_eq!(
                generator.has_payload(),
                expected_payload,
                "Test '{}': payload presence mismatch",
                name
            );
            assert_eq!(
                generator.has_topic_field(),
                expected_topic,
                "Test '{}': topic field presence mismatch",
                name
            );

            let actual_names = generator.param_names();
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
    fn test_edge_cases() {
        let edge_cases = vec![
            CodegenTestCase {
                name: "empty_struct",
                pattern: "sensors/+/data",
                struct_fields: quote! {},
                expected_checks: vec![
                    CodeCheck::TraitImpl("FromMqttMessage"),
                    CodeCheck::PayloadType("Vec < u8 >"), // Default payload
                    CodeCheck::Method("subscribe"),
                ],
            },
            CodegenTestCase {
                name: "only_topic_field",
                pattern: "status/+",
                struct_fields: quote! {
                    topic: Arc<TopicMatch>,
                },
                expected_checks: vec![
                    CodeCheck::FieldAssignment("topic"),
                    CodeCheck::PayloadType("Vec < u8 >"),
                ],
            },
            CodegenTestCase {
                name: "mixed_wildcards",
                pattern: "devices/+/{device_id}/status/#",
                struct_fields: quote! {
                    device_id: String,
                    payload: String,
                },
                expected_checks: vec![
                    CodeCheck::ParamExtraction { param_name: "device_id", index: 1 }, // Second wildcard
                    CodeCheck::FieldAssignment("device_id"),
                    CodeCheck::FieldAssignment("payload"),
                ],
            },
        ];

        for test_case in edge_cases {
            run_codegen_test(test_case);
        }
    }
}