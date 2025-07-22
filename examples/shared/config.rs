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
pub fn broker_url() -> String {
    // Load .env files (local overrides committed defaults)
    dotenv::dotenv().ok();
    if std::path::Path::new("examples/.env.local").exists() {
        dotenv::from_filename("examples/.env.local").ok();
    }
    
    env::var("MQTT_BROKER").unwrap_or_else(|_| {
        "mqtts://localhost:8883".to_string()
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
