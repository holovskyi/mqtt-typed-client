//! # MQTT Typed Client Macros
//!
//! This crate provides procedural macros for generating typed MQTT subscribers
//! with automatic topic parameter extraction and payload deserialization.
//!
//! ## Overview
//!
//! The main macro `mqtt_topic_subscriber` allows you to annotate a struct with
//! a topic pattern, automatically generating the necessary trait implementations
//! and helper methods for MQTT subscription handling.
//!
//! ## Features
//!
//! - **Topic Parameter Extraction**: Automatically extracts named parameters from MQTT topics
//! - **Type Safety**: Compile-time validation of struct fields against topic patterns
//! - **Flexible Payload Handling**: Support for custom payload types with automatic serialization
//! - **Optional Topic Access**: Include the full topic match information if needed
//! - **Generated Helper Methods**: Convenient subscription methods and pattern constants
//!
//! ## Quick Start
//!
//! ```rust
//! use mqtt_typed_client_macros::mqtt_topic_subscriber;
//! use std::sync::Arc;
//! use mqtt_typed_client::topic::topic_match::TopicMatch;
//!
//! #[derive(Debug)]
//! #[mqtt_topic_subscriber("sensors/{sensor_id}/temperature/{room}")]
//! struct TemperatureReading {
//!     sensor_id: u32,           // Extracted from {sensor_id} in topic
//!     room: String,             // Extracted from {room} in topic
//!     payload: f64,             // Message payload (temperature value)
//!     topic: Arc<TopicMatch>,   // Optional: full topic match info
//! }
//!
//! // Generated constants:
//! // TemperatureReading::TOPIC_PATTERN = "sensors/{sensor_id}/temperature/{room}"
//! // TemperatureReading::MQTT_PATTERN = "sensors/+/temperature/+"
//!
//! // Generated subscription method:
//! // let subscriber = TemperatureReading::subscribe(&client).await?;
//! ```
//!
//! ## Supported Field Types
//!
//! - **Topic Parameters**: Any field name matching a `{parameter}` in the topic pattern
//! - **`payload`**: The message payload, can be any deserializable type
//! - **`topic`**: Must be `Arc<TopicMatch>`, provides access to full topic information
//!
//! ## Topic Pattern Syntax
//!
//! - `{param_name}` - Named parameter that becomes a struct field
//! - `+` - Anonymous single-level wildcard (not extracted)
//! - `#` - Anonymous multi-level wildcard (not extracted, must be last)
//! - `{param_name:#}` - Named multi-level wildcard (extracted as string)

mod analysis;
mod codegen;

use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
use mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath;
use proc_macro::TokenStream;
use syn::{parse_macro_input, LitStr};

use crate::analysis::{StructAnalysisContext, TopicParam};

// Re-export key types for testing and advanced usage
// pub use analysis::{StructAnalysisContext, TopicParam};
// pub use codegen::{CodeGenerator, GenerationInfo};

/// Generate a typed MQTT subscriber from a struct and topic pattern
///
/// This macro analyzes the annotated struct and generates:
/// 1. `FromMqttMessage` trait implementation for message conversion
/// 2. Helper constants (`TOPIC_PATTERN`, `MQTT_PATTERN`)
/// 3. Async `subscribe()` method for easy subscription
///
/// ## Arguments
///
/// The macro takes a single string literal argument containing the topic pattern.
/// The pattern can include:
/// - Literal segments: `sensors`, `data`, `status`
/// - Named wildcards: `{sensor_id}`, `{room}`, `{device_type}`
/// - Anonymous wildcards: `+` (single level), `#` (multi-level, must be last)
///
/// ## Struct Requirements
///
/// The annotated struct must:
/// - Be a struct with named fields
/// - Only contain fields that correspond to:
///   - Topic parameters (matching `{param}` names in the pattern)
///   - `payload` field (optional, for message data)
///   - `topic` field (optional, must be `Arc<TopicMatch>`)
///
/// ## Generated Code
///
/// For a struct annotated with `#[mqtt_topic_subscriber("sensors/{id}/data")]`:
///
/// ```rust,ignore
/// impl<DE> FromMqttMessage<PayloadType, DE> for YourStruct {
///     fn from_mqtt_message(
///         topic: Arc<TopicMatch>,
///         payload: PayloadType,
///     ) -> Result<Self, MessageConversionError<DE>> {
///         // Parameter extraction and struct construction
///     }
/// }
///
/// impl YourStruct {
///     pub const TOPIC_PATTERN: &'static str = "sensors/{id}/data";
///     pub const MQTT_PATTERN: &'static str = "sensors/+/data";
///     
///     pub async fn subscribe<F>(client: &MqttClient<F>) -> Result<...> {
///         // Subscription logic
///     }
/// }
/// ```
///
/// ## Examples
///
/// ### Basic Usage
/// ```rust
/// #[derive(Debug)]
/// #[mqtt_topic_subscriber("sensors/{sensor_id}/temperature")]
/// struct TemperatureReading {
///     sensor_id: u32,
///     payload: f64,
/// }
/// ```
///
/// ### With Topic Information
/// ```rust
/// #[derive(Debug)]
/// #[mqtt_topic_subscriber("devices/{device_id}/status")]
/// struct DeviceStatus {
///     device_id: String,
///     payload: serde_json::Value,
///     topic: Arc<TopicMatch>,  // Access to full topic match
/// }
/// ```
///
/// ### Multiple Parameters
/// ```rust
/// #[derive(Debug)]
/// #[mqtt_topic_subscriber("buildings/{building}/floors/{floor}/rooms/{room}/sensors/{sensor_id}")]
/// struct SensorReading {
///     building: String,
///     floor: u32,
///     room: String,
///     sensor_id: u32,
///     payload: Vec<u8>,
/// }
/// ```
///
/// ### No Payload
/// ```rust
/// #[derive(Debug)]
/// #[mqtt_topic_subscriber("heartbeat/{service_name}")]
/// struct Heartbeat {
///     service_name: String,
///     // No payload field - will default to Vec<u8>
/// }
/// ```
///
/// ## Error Handling
///
/// The macro performs compile-time validation and will produce helpful error
/// messages for common issues:
///
/// - Unknown fields that don't match topic parameters
/// - Invalid topic patterns (e.g., `#` not at the end)
/// - Incorrect type for `topic` field
/// - Non-struct types or structs without named fields
///
/// ## Runtime Behavior
///
/// When messages are received:
/// 1. Topic is matched against the pattern
/// 2. Named parameters are extracted and parsed to their field types
/// 3. Payload is deserialized to the payload field type
/// 4. Struct is constructed with all extracted values
///
/// If parameter parsing fails (e.g., non-numeric string for `u32` field),
/// a `MessageConversionError` is returned.
#[proc_macro_attribute]
pub fn mqtt_topic_subscriber(args: TokenStream, input: TokenStream) -> TokenStream {
    let topic_pattern_str = parse_macro_input!(args as LitStr);
    let input_struct = parse_macro_input!(input as syn::DeriveInput);

    match generate_mqtt_subscriber(&topic_pattern_str, &input_struct) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Main orchestration function that coordinates analysis and code generation
///
/// This function ties together the analysis and code generation phases:
/// 1. Parse and validate the topic pattern
/// 2. Analyze the struct against the pattern
/// 3. Generate the complete implementation
///
/// # Error Handling
///
/// All errors are converted to `syn::Error` with appropriate spans and
/// descriptive messages for the best possible compile-time diagnostics.
fn generate_mqtt_subscriber(
    topic_pattern_str: &LitStr,
    input_struct: &syn::DeriveInput,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    // Parse and validate the topic pattern
    let topic_pattern = parse_topic_pattern(topic_pattern_str)?;
    
    // Analyze the struct against the pattern
    let context = analysis::StructAnalysisContext::analyze(input_struct, &topic_pattern)?;
    
    // Generate the complete implementation
    let generator = codegen::CodeGenerator::new(context);
    generator.generate_complete_implementation(input_struct, &topic_pattern)
}

/// Parse and validate a topic pattern string
///
/// Converts a string literal from the macro arguments into a validated
/// `TopicPatternPath`, checking for syntax errors and invalid wildcard usage.
///
/// # Arguments
/// * `topic_pattern_str` - String literal from macro arguments
///
/// # Returns
/// * `Ok(TopicPatternPath)` - Valid pattern ready for analysis
/// * `Err(syn::Error)` - Parse error with helpful message and correct span
///
/// # Validation
/// - Empty patterns are rejected
/// - `#` wildcards must be at the end
/// - Named parameters cannot be duplicated
/// - Wildcard syntax must be correct
fn parse_topic_pattern(
    topic_pattern_str: &LitStr,
) -> Result<TopicPatternPath, syn::Error> {
    TopicPatternPath::new_from_string(
        topic_pattern_str.value(),
        CacheStrategy::NoCache,
    )
    .map_err(|err| {
        syn::Error::new_spanned(
            topic_pattern_str,
            format!("Invalid topic pattern: {}", err),
        )
    })
}

/// Utility functions for advanced usage and testing
impl StructAnalysisContext {
    /// Create a context from components (useful for testing)
    #[doc(hidden)]
    pub fn from_components(
        payload_type: Option<syn::Type>,
        has_topic_field: bool,
        topic_params: Vec<TopicParam>,
    ) -> Self {
        Self {
            payload_type,
            has_topic_field,
            topic_params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    /// Test that the main macro orchestration works correctly
    #[test]
    fn test_generate_mqtt_subscriber_success() {
        let pattern_str: LitStr = parse_quote!("sensors/{sensor_id}/data");
        let test_struct: syn::DeriveInput = parse_quote! {
            struct TestStruct {
                sensor_id: u32,
                payload: String,
            }
        };

        let result = generate_mqtt_subscriber(&pattern_str, &test_struct);
        assert!(result.is_ok());

        let generated = result.unwrap();
        let code = generated.to_string();
        
        // Should contain all expected parts
        assert!(code.contains("struct TestStruct"));
        assert!(code.contains("FromMqttMessage"));
        assert!(code.contains("TOPIC_PATTERN"));
        assert!(code.contains("subscribe"));
    }

    #[test]
    fn test_generate_mqtt_subscriber_invalid_pattern() {
        let pattern_str: LitStr = parse_quote!("sensors/#/invalid");  // # not at end
        let test_struct: syn::DeriveInput = parse_quote! {
            struct TestStruct {
                payload: String,
            }
        };

        let result = generate_mqtt_subscriber(&pattern_str, &test_struct);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid topic pattern"));
    }

    #[test]
    fn test_generate_mqtt_subscriber_struct_validation_error() {
        let pattern_str: LitStr = parse_quote!("sensors/{sensor_id}/data");
        let test_struct: syn::DeriveInput = parse_quote! {
            struct TestStruct {
                unknown_field: String,  // Should cause error
            }
        };

        let result = generate_mqtt_subscriber(&pattern_str, &test_struct);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Unknown fields"));
    }

    #[test]
    fn test_parse_topic_pattern_success() {
        let pattern_str: LitStr = parse_quote!("sensors/{sensor_id}/temperature/{room}");
        let result = parse_topic_pattern(&pattern_str);
        
        assert!(result.is_ok());
        let pattern = result.unwrap();
        assert_eq!(pattern.topic_pattern(), "sensors/{sensor_id}/temperature/{room}");
        assert_eq!(pattern.mqtt_pattern(), "sensors/+/temperature/+");
    }

    #[test]
    fn test_parse_topic_pattern_empty() {
        let pattern_str: LitStr = parse_quote!("");
        let result = parse_topic_pattern(&pattern_str);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid topic pattern"));
    }

    #[test]
    fn test_parse_topic_pattern_invalid_hash_position() {
        let pattern_str: LitStr = parse_quote!("sensors/#/data");
        let result = parse_topic_pattern(&pattern_str);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid topic pattern"));
    }

    /// Integration test that ensures the full pipeline works
    #[test]
    fn test_full_integration() {
        let pattern_str: LitStr = parse_quote!("buildings/{building}/sensors/{sensor_id}/data");
        let test_struct: syn::DeriveInput = parse_quote! {
            struct BuildingSensor {
                building: String,
                sensor_id: u32,
                payload: f64,
                topic: Arc<TopicMatch>,
            }
        };

        let result = generate_mqtt_subscriber(&pattern_str, &test_struct);
        assert!(result.is_ok());

        let generated = result.unwrap();
        let code = generated.to_string();
        
        // Verify key components are present
        assert!(code.contains("struct BuildingSensor"));
        assert!(code.contains("impl < DE > :: mqtt_typed_client :: FromMqttMessage"));
        assert!(code.contains("buildings/{building}/sensors/{sensor_id}/data"));
        assert!(code.contains("buildings/+/sensors/+/data"));
        assert!(code.contains("extract_topic_parameter"));
        assert!(code.contains("building ,"));
        assert!(code.contains("sensor_id ,"));
        assert!(code.contains("payload ,"));
        assert!(code.contains("topic ,"));
    }

    /// Test utility function
    #[test]
    fn test_struct_analysis_context_from_components() {
        let payload_type: syn::Type = parse_quote!(String);
        let topic_params = vec![
            TopicParam { name:"sensor_id".to_string(), wildcard_index : 0 },
			TopicParam { name:"room".to_string(), wildcard_index : 1 },
        ];

        let context = StructAnalysisContext::from_components(
            Some(payload_type),
            true,
            topic_params,
        );

        assert!(context.payload_type.is_some());
        assert!(context.has_topic_field);
        assert_eq!(context.topic_params.len(), 2);
    }
}