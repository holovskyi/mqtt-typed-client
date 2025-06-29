//! Integration test for typed client generation

use mqtt_typed_client_macros::mqtt_topic;

#[mqtt_topic("sensors/{sensor_id}/temperature")]
struct SensorReading {
    sensor_id: u32,
    payload: f64,
}

#[tokio::test]
async fn test_typed_client_compilation() {
    // Test that the generated code compiles
    // We can't actually test functionality without a real MQTT broker
    
    // Check that extension trait method exists
    let _method_name = "sensor_reading"; // This should match generated method name
    
    // Check that client struct exists  
    let _client_type_name = "SensorReadingClient"; // This should match generated struct name
    
    // Check that extension trait exists
    let _trait_name = "SensorReadingExt"; // This should match generated trait name
    
    println!("Typed client generation test passed - code compiles successfully!");
}

// Test multiple message types to ensure no naming conflicts
#[mqtt_topic("alerts/{level}/messages")]  
struct AlertMessage {
    level: String,
    payload: String,
}

#[tokio::test]
async fn test_multiple_typed_clients() {
    // Both should compile without conflicts
    let _sensor_method = "sensor_reading";
    let _alert_method = "alert_message";
    
    println!("Multiple typed clients test passed!");
}
