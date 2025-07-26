//! # MQTT Typed Client
//!
#![doc = include_str!("../README.md")]
//!
//! ## API Reference
//!
//! Key traits and modules:
//! - [`MessageSerializer`] - Custom serialization trait
//! - [`prelude`] - Convenient imports for common use cases
//! - [`info`] - Library metadata and version info
//!
//! ## See Also
//!
//! - [`crate::comparison`] - Detailed comparison with rumqttc
//! - [`crate::examples`] - Complete usage examples with source code

pub use mqtt_typed_client_core::*;
#[cfg(feature = "macros")]
pub use mqtt_typed_client_macros::*;

pub mod prelude {
	//! Convenient imports for common use cases

	pub use mqtt_typed_client_core::structured::*;
	pub use mqtt_typed_client_core::{
		BincodeSerializer, ClientSettings, MessageSerializer, MqttClient,
		MqttClientConfig, MqttClientError, MqttConnection, MqttOptions,
		MqttPublisher, MqttSubscriber, QoS, Result, SubscriptionBuilder,
		TypedLastWill,
	};
	#[cfg(feature = "macros")]
	pub use mqtt_typed_client_macros::*;
}

pub mod comparison {
	//! Detailed comparison with rumqttc
	//!
	//! This module provides comprehensive side-by-side comparison
	//! between mqtt-typed-client and the underlying rumqttc library.
	//!
	#![doc = include_str!("../docs/COMPARISON_WITH_RUMQTTC.md")]
}

pub mod examples {
	//! Complete usage examples with source code
	//!
	//! This module contains comprehensive examples demonstrating
	//! various features and use cases of mqtt-typed-client.
	//!
	#![doc = include_str!("../examples/README.md")]

	pub mod example_000_hello_world {
		//! # Hello World Example
		//!
		//! Basic publish/subscribe with macros - demonstrates the core functionality
		//! of mqtt-typed-client with type-safe topic handling.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 000_hello_world
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/000_hello_world.rs")]
		//! ```
	}

	pub mod example_001_ping_pong {
		//! # Ping Pong Example
		//!
		//! Multi-client communication demonstrating bidirectional message exchange
		//! and concurrent MQTT operations.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 001_ping_pong
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/001_ping_pong.rs")]
		//! ```
	}

	pub mod example_002_configuration {
		//! # Configuration Example
		//!
		//! Advanced client configuration showing how to customize MQTT settings,
		//! QoS levels, and connection parameters.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 002_configuration
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/002_configuration.rs")]
		//! ```
	}

	pub mod example_003_hello_world_lwt {
		//! # Last Will & Testament Example
		//!
		//! Demonstrates MQTT Last Will & Testament functionality for handling
		//! unexpected client disconnections.
		//!
		//! ## Usage
		//! ```bash
		//! # Terminal 1: Start subscriber
		//! cargo run --example 003_hello_world_lwt
		//!
		//! # Terminal 2: Run publisher (sends message then crashes)
		//! cargo run --example 003_hello_world_lwt -- --publisher
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/003_hello_world_lwt.rs")]
		//! ```
	}

	pub mod example_004_hello_world_tls {
		//! # TLS/SSL Connections Example
		//!
		//! Secure MQTT connections using TLS/SSL with custom certificate handling
		//! for development and production environments.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 004_hello_world_tls
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/004_hello_world_tls.rs")]
		//! ```
	}

	pub mod example_005_hello_world_serializers {
		//! # Custom Serializers Example
		//!
		//! Demonstrates how to implement and use custom message serializers
		//! beyond the built-in formats.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 005_hello_world_serializers
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/005_hello_world_serializers.rs")]
		//! ```
	}

	pub mod example_006_retain_and_clear {
		//! # Retained Messages Example
		//!
		//! MQTT retained message functionality with multiple clients connecting
		//! at different times to showcase broker message persistence.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 006_retain_and_clear
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/006_retain_and_clear.rs")]
		//! ```
	}

	pub mod example_007_custom_patterns {
		//! # Custom Topic Patterns Example
		//!
		//! Advanced topic pattern usage showing how to override default patterns
		//! for environment-specific routing and multi-tenant applications.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 007_custom_patterns
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/007_custom_patterns.rs")]
		//! ```
	}

	pub mod example_008_modular_example {
		//! # Modular Application Structure Example
		//!
		//! Production-ready application structure showing how to organize
		//! MQTT applications with multiple modules and clean separation of concerns.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 008_modular_example
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/008_modular_example.rs")]
		//! ```
	}

	pub mod example_100_all_serializers_demo {
		//! # Complete Serializer Test Suite
		//!
		//! Comprehensive example testing all 8 available serializers
		//! with full publish/subscribe verification.
		//!
		//! ## Usage
		//! ```bash
		//! cargo run --example 100_all_serializers_demo --all-features
		//! ```
		//!
		//! ## Source Code
		//!
		//! ```rust
		#![doc = include_str!("../examples/100_all_serializers_demo.rs")]
		//! ```
	}
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod info {
	//! Library metadata and version information

	pub const NAME: &str = env!("CARGO_PKG_NAME");
	pub const VERSION: &str = env!("CARGO_PKG_VERSION");
	pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
	pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
	pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
	pub const HAS_MACROS: bool = cfg!(feature = "macros");

	/// Returns a formatted version string with feature information
	pub fn version_string() -> String {
		if HAS_MACROS {
			format!("{VERSION} (with macros)")
		} else {
			format!("{VERSION} (core only)")
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_version_is_valid() {
		assert!(VERSION.chars().next().unwrap().is_ascii_digit());
	}

	#[test]
	fn test_info_module() {
		assert_eq!(info::VERSION, VERSION);
		assert!(!info::version_string().is_empty());
	}

	#[cfg(feature = "macros")]
	#[test]
	fn test_macros_feature_enabled() {
		assert!(info::HAS_MACROS);
		assert!(info::version_string().contains("with macros"));
	}

	#[cfg(not(feature = "macros"))]
	#[test]
	fn test_macros_feature_disabled() {
		assert!(!info::HAS_MACROS);
		assert!(info::version_string().contains("core only"));
	}
}

#[cfg(feature = "macros")]
#[doc = ""]
#[doc = "## Procedural Macros"]
#[doc = ""]
#[doc = "This build includes procedural macros for enhanced type safety and \
         code generation."]
#[doc = "See the `mqtt_typed_client_macros` documentation for detailed macro \
         usage."]
pub mod _macro_docs {}

#[cfg(not(feature = "macros"))]
#[doc = ""]
#[doc = "## Core Only Build"]
#[doc = ""]
#[doc = "This build does not include procedural macros. To enable macros, add \
         the 'macros' feature:"]
#[doc = "```toml"]
#[doc = "[dependencies]"]
#[doc = "mqtt-typed-client = { version = \"0.1\", features = [\"macros\"] }"]
#[doc = "```"]
pub mod _core_only_docs {}
