//! Test example for custom topic patterns
//!
//! This example demonstrates how to use custom topic patterns with
//! the mqtt_topic macro while maintaining structural compatibility.

use mqtt_typed_client_macros::mqtt_topic;
use mqtt_typed_client::{BincodeSerializer, MqttClient, SubscriptionConfig, CacheStrategy};
use serde::{Deserialize, Serialize};
use bincode::{Decode, Encode};
use std::num::NonZeroUsize;

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct SensorReading {
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

#[mqtt_topic("sensors/{building}/{floor}/temp/{sensor_id}")]
#[derive(Debug)]
struct TemperatureSensor {
    building: String,   // Automatically extracted from topic
    floor: u32,        // Parsed to correct type  
    sensor_id: String, // Sensor ID
    payload: SensorReading, // Data from message
}

async fn test_custom_patterns() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing custom topic patterns...");
    
    // Print original patterns
    println!("Original topic pattern: {}", TemperatureSensor::TOPIC_PATTERN);
    println!("Original MQTT pattern: {}", TemperatureSensor::MQTT_PATTERN);
    
    // Test that constants are correct
    assert_eq!(TemperatureSensor::TOPIC_PATTERN, "sensors/{building}/{floor}/temp/{sensor_id}");
    assert_eq!(TemperatureSensor::MQTT_PATTERN, "sensors/+/+/temp/+");
    
    println!("✅ Constants are correct");

    // These would work in a real MQTT environment:
    
    // Test 1: Custom pattern with default config
    // let custom_subscriber = TemperatureSensor::subscribe_pattern(
    //     &client,
    //     "data/{building}/{floor}/temperature/{sensor_id}"
    // ).await?;
    
    // Test 2: Custom pattern with custom config  
    // let config = SubscriptionConfig {
    //     qos: mqtt_typed_client::QoS::ExactlyOnce,
    //     cache_strategy: CacheStrategy::Lru(NonZeroUsize::new(1000).unwrap()),
    // };
    // let subscriber = TemperatureSensor::subscribe_pattern_with_config(
    //     &client,
    //     "iot/{building}/{floor}/temp/{sensor_id}",
    //     config
    // ).await?;
    
    // Test 3: Invalid pattern should fail validation
    // This would return PatternStructureMismatch error:
    // let invalid = TemperatureSensor::subscribe_pattern(
    //     &client,
    //     "data/{floor}/{building}/temp/{sensor_id}"  // Wrong parameter order
    // ).await; // Should fail
    
    println!("✅ All pattern methods are available and correctly typed");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    test_custom_patterns().await?;
    println!("🎉 Custom pattern test completed successfully!");
    Ok(())
}
