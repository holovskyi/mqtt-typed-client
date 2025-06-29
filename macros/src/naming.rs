//! Naming utilities for typed client generation

use quote::format_ident;

/// Generated names for typed client components
#[derive(Debug, Clone)]
pub struct TypedClientNames {
    /// snake_case method name: "sensor_message"  
    pub method_name: String,
    /// Client struct name: "SensorMessageClient"
    pub client_struct: syn::Ident,
    /// Extension trait name: "SensorMessageExt"
    pub extension_trait: syn::Ident,
}

impl TypedClientNames {
    /// Generate all names from struct identifier
    pub fn from_struct_name(struct_name: &syn::Ident) -> Self {
        let base_name = struct_name.to_string();
        let method_name = to_snake_case(&base_name);
        let client_struct = format_ident!("{}Client", base_name);
        let extension_trait = format_ident!("{}Ext", base_name);

        Self {
            method_name,
            client_struct,
            extension_trait,
        }
    }
}

/// Convert PascalCase to snake_case
fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let chars = input.chars().peekable();
    
    for ch in chars {
        if ch.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;

    #[test]
    fn test_snake_case_conversion() {
        assert_eq!(to_snake_case("SensorMessage"), "sensor_message");
        assert_eq!(to_snake_case("DeviceAlert"), "device_alert");
        assert_eq!(to_snake_case("HTTPRequest"), "h_t_t_p_request");
        assert_eq!(to_snake_case("Message"), "message");
        assert_eq!(to_snake_case("IOEvent"), "i_o_event");
    }

    #[test]
    fn test_typed_client_names_generation() {
        let struct_name = format_ident!("SensorMessage");
        let names = TypedClientNames::from_struct_name(&struct_name);
        
        assert_eq!(names.method_name, "sensor_message");
        assert_eq!(names.client_struct.to_string(), "SensorMessageClient");
        assert_eq!(names.extension_trait.to_string(), "SensorMessageExt");
    }

    #[test]
    fn test_complex_names() {
        let struct_name = format_ident!("DeviceTemperatureAlert");
        let names = TypedClientNames::from_struct_name(&struct_name);
        
        assert_eq!(names.method_name, "device_temperature_alert");
        assert_eq!(names.client_struct.to_string(), "DeviceTemperatureAlertClient");
        assert_eq!(names.extension_trait.to_string(), "DeviceTemperatureAlertExt");
    }
}
