//! # MQTT Typed Client Macros
//!
//! This crate provides procedural macros for generating typed MQTT subscribers
//! and publishers with automatic topic parameter extraction and payload serialization.
//!
//! ## Overview
//!
//! The main macro `mqtt_topic` allows you to annotate a struct with
//! a topic pattern, automatically generating the necessary trait implementations
//! and helper methods for MQTT subscription and publishing.
//!
//! ## Features
//!
//! - **Topic Parameter Extraction**: Automatically extracts named parameters from MQTT topics
//! - **Type Safety**: Compile-time validation of struct fields against topic patterns
//! - **Flexible Payload Handling**: Support for custom payload types with automatic serialization
//! - **Optional Topic Access**: Include the full topic match information if needed
//! - **Dual Mode Generation**: Generate subscriber, publisher, or both methods
//! - **Generated Helper Methods**: Convenient subscription and publishing methods with pattern constants
//!
//! ## Quick Start
//!
//! ```rust
//! use mqtt_typed_client_macros::mqtt_topic;
//! use std::sync::Arc;
//! use mqtt_typed_client::topic::topic_match::TopicMatch;
//!
//! #[derive(Debug)]
//! #[mqtt_topic("sensors/{sensor_id}/temperature/{room}")]
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
//! // Generated methods:
//! // let subscriber = TemperatureReading::subscribe(&client).await?;
//! // TemperatureReading::publish(&client, sensor_id, room, &data).await?;
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
//!
//! ### ⚠️ Publisher Limitations
//!
//! **Multi-level wildcards (`#` or `{param:#}`) are not supported for publishers**
//! because they represent variable-length topic segments that cannot be constructed
//! from fixed parameters.
//!
//! ```rust
//! use mqtt_typed_client_macros::mqtt_topic;
//! 
//! // ✅ This works for both subscriber and publisher
//! #[mqtt_topic("sensors/{sensor_id}/data")]
//! struct SensorData { sensor_id: u32, payload: f64 }
//!
//! // ✅ This works for subscriber only
//! #[mqtt_topic("alerts/{category}/#", subscriber)]
//! struct Alert { category: String, payload: String }
//!
//! // ❌ This will cause a compile error
//! // #[mqtt_topic("events/{event_type}/{details:#}")]
//! // struct Event { /* ... */ }
//! 
//! // 🔧 Solution: Use separate structs
//! #[mqtt_topic("events/{event_type}/{details:#}", subscriber)]  
//! struct EventReceived { event_type: String, details: String, payload: Vec<u8> }
//!
//! #[mqtt_topic("events/{event_type}", publisher)]
//! struct EventToSend { event_type: String, payload: Vec<u8> }
//! ```

mod analysis;
#[cfg(test)]
mod analysis_test;
mod codegen;
#[cfg(test)]
mod codegen_test;
mod codegen_typed_client;
#[cfg(test)]
mod lib_test;
mod naming;

use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
use mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath;
use proc_macro::TokenStream;
use syn::{LitStr, parse::Parser, parse_macro_input};

// Re-export key types for testing and advanced usage
// pub use analysis::{StructAnalysisContext, TopicParam};
// pub use codegen::{CodeGenerator, GenerationInfo};

/// Generate a typed MQTT subscriber and/or publisher from a struct and topic pattern
///
/// This macro analyzes the annotated struct and generates:
/// 1. `FromMqttMessage` trait implementation for message conversion (if subscriber enabled)
/// 2. Helper constants (`TOPIC_PATTERN`, `MQTT_PATTERN`)
/// 3. Async `subscribe()` method for easy subscription (if subscriber enabled)
/// 4. `publish()` and `get_publisher()` methods for publishing (if publisher enabled)
///
/// ## Arguments
///
/// The macro takes a topic pattern string and optional mode flags:
/// - `#[mqtt_topic("pattern")]` - Generate both subscriber and publisher (default)
/// - `#[mqtt_topic("pattern", subscriber)]` - Generate only subscriber
/// - `#[mqtt_topic("pattern", publisher)]` - Generate only publisher
/// - `#[mqtt_topic("pattern", subscriber, publisher)]` - Generate both (explicit)
///
/// The pattern can include:
/// - Literal segments: `sensors`, `data`, `status`
/// - Named wildcards: `{sensor_id}`, `{room}`, `{device_type}`
/// - Anonymous wildcards: `+` (single level), `#` (multi-level, subscriber-only)
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
/// For a struct annotated with `#[mqtt_topic("sensors/{id}/data")]`:
///
/// ```rust,ignore
/// // Subscriber functionality (if enabled)
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
///     // Subscriber methods (if enabled)
///     pub async fn subscribe<F>(client: &MqttClient<F>) -> Result<...> {
///         // Subscription logic
///     }
///     
///     // Publisher methods (if enabled)
///     pub async fn publish<F>(client: &MqttClient<F>, id: ParamType, data: &PayloadType) -> Result<...> {
///         // Publishing logic
///     }
///     
///     pub fn get_publisher<F>(client: &MqttClient<F>, id: ParamType) -> Result<...> {
///         // Publisher creation
///     }
/// }
/// ```
///
/// ## Examples
///
/// ### Basic Usage (Both Modes)
/// ```rust
/// # use mqtt_typed_client_macros::mqtt_topic;
/// #[derive(Debug)]
/// #[mqtt_topic("sensors/{sensor_id}/temperature")]
/// struct TemperatureReading {
///     sensor_id: u32,
///     payload: f64,
/// }
/// ```
///
/// ### Subscriber Only
/// ```rust
/// # use mqtt_typed_client_macros::mqtt_topic;
/// # use std::sync::Arc;
/// # use mqtt_typed_client::topic::topic_match::TopicMatch;
/// #[derive(Debug)]
/// #[mqtt_topic("devices/{device_id}/status/#", subscriber)]
/// struct DeviceStatus {
///     device_id: String,
///     payload: Vec<u8>,
///     topic: Arc<TopicMatch>,  // Access to full topic match
/// }
/// ```
///
/// ### Publisher Only
/// ```rust
/// # use mqtt_typed_client_macros::mqtt_topic;
/// #[derive(Debug)]
/// #[mqtt_topic("commands/{service}/{action}", publisher)]
/// struct Command {
///     service: String,
///     action: String,
///     payload: String,
/// }
/// ```
///
/// ### Multiple Parameters
/// ```rust
/// # use mqtt_typed_client_macros::mqtt_topic;
/// #[derive(Debug)]
/// #[mqtt_topic("buildings/{building}/floors/{floor}/rooms/{room}/sensors/{sensor_id}")]
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
/// # use mqtt_typed_client_macros::mqtt_topic;
/// #[derive(Debug)]
/// #[mqtt_topic("heartbeat/{service_name}")]
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
/// - Publisher mode with `#` wildcards (not supported)
///
/// ## Runtime Behavior
///
/// ### Subscriber
/// When messages are received:
/// 1. Topic is matched against the pattern
/// 2. Named parameters are extracted and parsed to their field types
/// 3. Payload is deserialized to the payload field type
/// 4. Struct is constructed with all extracted values
///
/// ### Publisher
/// When publishing messages:
/// 1. Topic parameters are provided as method arguments
/// 2. Topic string is constructed from the pattern
/// 3. Payload is serialized and published
///
/// If parameter parsing fails (e.g., non-numeric string for `u32` field),
/// a `MessageConversionError` is returned.
#[proc_macro_attribute]
pub fn mqtt_topic(args: TokenStream, input: TokenStream) -> TokenStream {
	let input_struct = parse_macro_input!(input as syn::DeriveInput);

	let macro_args = match parse_macro_args(args) {
		| Ok(args) => args,
		| Err(err) => return err.to_compile_error().into(),
	};

	match generate_mqtt_code(macro_args, &input_struct) {
		| Ok(tokens) => tokens.into(),
		| Err(err) => err.to_compile_error().into(),
	}
}

/// Main orchestration function that coordinates analysis and code generation
///
/// Ties together the analysis and code generation phases.
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
fn generate_mqtt_code(
	macro_args: MacroArgs,
	input_struct: &syn::DeriveInput,
) -> Result<proc_macro2::TokenStream, syn::Error> {
	// Analyze the struct against the pattern
	let context = analysis::StructAnalysisContext::analyze(
		input_struct,
		&macro_args.pattern,
	)?;

	// Generate the complete implementation
	let generator = codegen::CodeGenerator::new(context, macro_args);
	generator.generate_complete_implementation(input_struct)
}

/// Parse macro arguments: pattern string and optional generation mode flags
///
/// Supports: `pattern`, `(pattern, subscriber)`, `(pattern, publisher)`, `(pattern, subscriber, publisher)`
fn parse_macro_args(args: TokenStream) -> Result<MacroArgs, syn::Error> {
	let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
	let args = parser.parse(args)?;

	if args.is_empty() {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"mqtt_topic macro requires at least a topic pattern string",
		));
	}

	// First argument must be the pattern string
	let pattern = match &args[0] {
		| syn::Expr::Lit(syn::ExprLit {
			lit: syn::Lit::Str(lit_str),
			..
		}) => lit_str.clone(),
		| _ => {
			return Err(syn::Error::new_spanned(
				&args[0],
				"First argument must be a string literal containing the topic \
				 pattern",
			));
		}
	};

	let topic_pattern = parse_topic_pattern(&pattern)?;

	// Default: generate both
	let mut generate_subscriber = true;
	let mut generate_publisher = true;
	let mut explicit_modes = Vec::new();

	// Parse optional mode flags
	for arg in args.iter().skip(1) {
		match arg {
			| syn::Expr::Path(expr_path)
				if expr_path.path.is_ident("subscriber") =>
			{
				explicit_modes.push("subscriber");
			}
			| syn::Expr::Path(expr_path)
				if expr_path.path.is_ident("publisher") =>
			{
				explicit_modes.push("publisher");
			}
			| _ => {
				return Err(syn::Error::new_spanned(
					arg,
					"Invalid argument. Only 'subscriber' and 'publisher' \
					 flags are allowed",
				));
			}
		}
	}

	if explicit_modes.len() > 2 {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"Too many arguments. Only 'subscriber' and 'publisher' flags are \
			 allowed",
		));
	}
	// Apply explicit modes if any were specified
	if !explicit_modes.is_empty() {
		generate_subscriber = explicit_modes.contains(&"subscriber");
		generate_publisher = explicit_modes.contains(&"publisher");
	}

	// Validate configuration
	if !(generate_subscriber || generate_publisher) {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"At least one of 'subscriber' or 'publisher' must be enabled",
		));
	}

	// Check for multi-level wildcards if publisher is requested
	if generate_publisher && topic_pattern.contains_hash() {
		return Err(syn::Error::new_spanned(
			&pattern,
			format!(
				"Cannot generate publisher methods for patterns with '#' wildcards.\n\n\
				 Solutions:\n\
				   • Use subscriber-only mode: #[mqtt_topic(\"{}\", subscriber)]\n\
				   • Create separate structs for different purposes:\n\n\
				     #[mqtt_topic(\"{}\", subscriber)]\n\
				     struct EventReceived {{ /* fields */ }}\n\n\
				     #[mqtt_topic(\"events/{{category}}\", publisher)]\n\
				     struct EventToSend {{ /* fields */ }}\n\n\
				 Why: Publishers need concrete topic strings, but '#' represents \n\
				 variable-length paths that cannot be determined at compile time.",
				topic_pattern.topic_pattern(),
				topic_pattern.topic_pattern()
			),
		));
	}

	let macro_args = MacroArgs {
		pattern: topic_pattern,
		generate_subscriber,
		generate_publisher,
		generate_typed_client: true,  // Enable by default
	};

	Ok(macro_args)
}

/// Parse and validate a topic pattern string
///
/// Converts string literal into validated TopicPatternPath with syntax validation.
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

/// Macro configuration arguments
#[derive(Debug)]
struct MacroArgs {
	pattern: TopicPatternPath,
	generate_subscriber: bool,
	generate_publisher: bool,
	generate_typed_client: bool,
}

#[cfg(test)]
mod test_helpers {
	use super::*;
	
	pub fn create_test_macro_args() -> MacroArgs {
		let topic_pattern = TopicPatternPath::new_from_string(
			"sensors/{sensor_id}/temp".to_string(),
			CacheStrategy::NoCache,
		).unwrap();
		
		MacroArgs {
			pattern: topic_pattern,
			generate_subscriber: true,
			generate_publisher: true,
			generate_typed_client: true,
		}
	}
}


