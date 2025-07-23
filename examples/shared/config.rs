use std::env;
use uuid::Uuid;

/// Get MQTT broker URL from environment variable or use default
/// 
/// Loads configuration from .env files in this order:
/// 1. examples/.env.local (if exists, ignored by git)
/// 2. examples/.env (committed defaults)
/// 3. Environment variables
/// 4. Hardcoded default
/// 
/// # Examples
/// - `MQTT_BROKER=mqtt://broker.hivemq.com:1883` - Public broker
/// - `MQTT_BROKER=mqtts://localhost:8883` - Local TLS broker (default)
#[allow(dead_code)]
pub fn broker_url() -> String {
    // Load .env files with explicit paths (working dir is project root)
    dotenv::from_filename("examples/.env").ok();
    if std::path::Path::new("examples/.env.local").exists() {
        dotenv::from_filename("examples/.env.local").ok();
    }
    
    env::var("MQTT_BROKER").unwrap_or_else(|_| {
        "mqtt://localhost:1883".to_string()
    })
}

/// Generate unique client ID with given prefix
/// 
/// Creates a client ID by combining the prefix with a random UUID.
/// Useful to avoid client ID conflicts when running multiple examples.
/// 
/// # Arguments
/// * `prefix` - String prefix for the client ID
/// 
/// # Examples
/// - `get_client_id("hello_world")` → `"hello_world_a1b2c3d4"`
/// - `get_client_id("sensor")` → `"sensor_e5f6g7h8"`
#[allow(dead_code)]
pub fn get_client_id(prefix: &str) -> String {
    let uuid = Uuid::new_v4().to_string();
    let short_uuid = &uuid[..8]; // Take first 8 characters
    format!("{prefix}_{short_uuid}")
}

/// Build complete MQTT URL with client ID
/// 
/// Combines broker URL with client ID parameter.
/// Handles both cases: URL already has query parameters or doesn't.
/// 
/// # Arguments
/// * `client_id_prefix` - Prefix for generating unique client ID
/// 
/// # Examples
/// - `build_url("hello_world")` → `"mqtts://localhost:8883?client_id=hello_world_a1b2c3d4"`
/// - With custom broker: `"mqtt://broker.com:1883?client_id=sensor_e5f6g7h8"`
#[allow(dead_code)]
pub fn build_url(client_id_prefix: &str) -> String {
    let base_url = broker_url();
    let client_id = get_client_id(client_id_prefix);
    
    if base_url.contains('?') {
        // URL already has query parameters, append with &
        format!("{base_url}&client_id={client_id}")
    } else {
        // URL has no query parameters, start with ?
        format!("{base_url}?client_id={client_id}")
    }
}

/// Print helpful connection error message with troubleshooting tips
/// 
/// Helper function to display user-friendly error messages with actionable advice.
/// Keeps examples clean while providing comprehensive troubleshooting guidance.
/// Automatically detects example name from the current executable.
/// 
/// # Arguments
/// * `url` - The MQTT broker URL that failed to connect
/// * `error` - The connection error that occurred
#[allow(dead_code)]
pub fn print_connection_error(url: &str, error: &dyn std::error::Error) {
    let example_name = get_example_name();
    
    eprintln!("❌ Connection failed to: {url}");
    eprintln!("   Error: {error}");
    eprintln!();
    eprintln!("💡 Troubleshooting:");
    
    if url.contains("localhost") {
        eprintln!("   • Start local MQTT broker: cd dev && docker-compose up -d");
        eprintln!("   • Check if broker is running: docker-compose ps");
    } else {
        eprintln!("   • Check network connection");
        eprintln!("   • Verify broker URL is correct");
    }
    
    eprintln!("   • Try a public broker: MQTT_BROKER=\"mqtt://broker.hivemq.com:1883\" cargo run --example {example_name}");
    eprintln!("   • Enable debug logs: RUST_LOG=debug cargo run --example {example_name}");
}

/// Parse MQTT broker URL to extract host and port
/// 
/// Extracts host and port from MQTT broker URL for use with MqttClientConfig.
/// Handles both mqtt:// and mqtts:// schemes with appropriate default ports.
/// 
/// # Examples
/// - `get_mqtt_broker_host_port()` with MQTT_BROKER="mqtt://localhost:1883" → ("localhost", 1883)
/// - `get_mqtt_broker_host_port()` with MQTT_BROKER="mqtts://localhost:8883" → ("localhost", 8883)
/// - `get_mqtt_broker_host_port()` with MQTT_BROKER="mqtt://broker.hivemq.com:1883" → ("broker.hivemq.com", 1883)
#[allow(dead_code)]
pub fn get_mqtt_broker_host_port() -> (String, u16) {
    let url = broker_url();
    
    // Simple URL parsing for common MQTT URL formats
    // For production use, consider using a proper URL parsing library
    
    if let Some(without_scheme) = url.strip_prefix("mqtt://").or_else(|| url.strip_prefix("mqtts://")) {
        let is_tls = url.starts_with("mqtts://");
        let default_port = if is_tls { 8883 } else { 1883 };
        
        if let Some((host, port_str)) = without_scheme.split_once(':') {
            // Extract port, ignoring query parameters
            let port_part = port_str.split('?').next().unwrap_or(port_str);
            if let Ok(port) = port_part.parse::<u16>() {
                return (host.to_string(), port);
            }
        } else {
            // No port specified, use default
            let host = without_scheme.split('?').next().unwrap_or(without_scheme);
            return (host.to_string(), default_port);
        }
    }
    
    // Fallback if parsing fails
    ("localhost".to_string(), 1883)
}

/// Extract example name from current executable path
/// 
/// Attempts to get the example name from the current executable.
/// Falls back to "example" if detection fails.
#[allow(dead_code)]
fn get_example_name() -> String {
    std::env::args()
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_stem()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "example".to_string())
}
