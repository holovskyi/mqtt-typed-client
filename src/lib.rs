//! # MQTT Typed Client
//! 
//! A type-safe MQTT client library with optional procedural macros for enhanced developer experience.
//! 
//! This crate provides a high-level, type-safe interface for MQTT communication with support for
//! structured topic patterns, automatic message serialization, and optional procedural macros
//! for compile-time topic validation and code generation.
//! 
//! ## Features
//! 
//! - **Type-safe**: Strongly typed topic patterns and message handling
//! - **Async/await**: Built on tokio for high performance async operations
//! - **Flexible routing**: Advanced topic matching and message routing
//! - **Serialization**: Automatic JSON/binary message serialization
//! - **Procedural macros**: Optional compile-time code generation (default feature)
//! - **Connection management**: Automatic reconnection and last will support
//! 
//! ## Quick Start
//! TODO


pub use mqtt_typed_client_core::*;

#[cfg(feature = "macros")]
pub use mqtt_typed_client_macros::*;

pub mod prelude {
    //! Convenient imports for common use cases
    
    pub use mqtt_typed_client_core::{
        MqttClient,
        MqttClientConfig,
        ClientSettings,
        MqttPublisher,
        MqttSubscriber,
        SubscriptionBuilder,
        MqttConnection,
        TypedLastWill,
        MessageSerializer,
        BincodeSerializer,
        MqttClientError,
        Result,
        QoS,
        MqttOptions,
    };
    
    pub use mqtt_typed_client_core::structured::*;
    
    #[cfg(feature = "macros")]
    pub use mqtt_typed_client_macros::*;
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod info {
    pub const NAME: &str = env!("CARGO_PKG_NAME");
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
    pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
    pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
    pub const HAS_MACROS: bool = cfg!(feature = "macros");
    
    pub fn version_string() -> String {
        if HAS_MACROS {
            format!("{} (with macros)", VERSION)
        } else {
            format!("{} (core only)", VERSION)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_is_valid() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.chars().next().unwrap().is_ascii_digit());
    }
    
    #[test]
    fn test_info_module() {
        assert_eq!(info::VERSION, VERSION);
        assert!(!info::NAME.is_empty());
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
#[doc = "This build includes procedural macros for enhanced type safety and code generation."]
#[doc = "See the `mqtt_typed_client_macros` documentation for detailed macro usage."]
pub mod _macro_docs {}

#[cfg(not(feature = "macros"))]
#[doc = ""]
#[doc = "## Core Only Build"]
#[doc = ""]
#[doc = "This build does not include procedural macros. To enable macros, add the 'macros' feature:"]
#[doc = "```toml"]
#[doc = "[dependencies]"]
#[doc = "mqtt-typed-client = { version = \"0.1\", features = [\"macros\"] }"]
#[doc = "```"]
pub mod _core_only_docs {}