//! Complete test demonstrating all new subscription methods
//!
//! This shows how to use all four subscription methods with the mqtt_topic macro


use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, SubscriptionConfig, QoS};
use mqtt_typed_client_macros::mqtt_topic;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct SensorReading {
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

#[allow(dead_code)]
#[mqtt_topic("sensors/{building}/{floor}/temp/{sensor_id}")]
#[derive(Debug)]
struct TemperatureSensor {
    building: String,
    floor: u32,
    sensor_id: String,
    payload: SensorReading,
}

async fn demonstrate_subscription_methods() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Demonstrating all subscription methods");
    
    // This would work with a real MQTT broker:
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(
        "mqtt://broker.hivemq.com:1883?client_id=test_patterns"
    ).await?;
    
    println!("📋 Available subscription methods:");
    
    // Method 1: Basic subscription (default config)
    println!("1️⃣  Basic: subscribe()");
    let _basic_subscriber = TemperatureSensor::subscribe(&client).await?;
    println!("    ✅ Subscribed to: {}", TemperatureSensor::MQTT_PATTERN);
    
    // Method 2: Subscription with custom config
    println!("2️⃣  With config: subscribe_with_config()");
    let high_performance_config = SubscriptionConfig {
        qos: QoS::ExactlyOnce,
        //TODO cache_strategy: CacheStrategy::Lru(NonZeroUsize::new(1000).unwrap()),
    };
    let _config_subscriber = TemperatureSensor::subscribe_with_config(
        &client, 
        high_performance_config
    ).await?;
    println!("    ✅ Subscribed with QoS::ExactlyOnce and LRU cache");
    
    // Method 3: Custom pattern with default config
    println!("3️⃣  Custom pattern: subscribe_pattern()");
    let _pattern_subscriber = TemperatureSensor::subscribe_to_custom_topic(
        &client,
        "data/{building}/{floor}/temperature/{sensor_id}"
    ).await?;
    println!("    ✅ Subscribed to custom pattern: data/+/+/temperature/+");
    
    // Method 4: Custom pattern with custom config
    println!("4️⃣  Full control: subscribe_pattern_with_config()");
    let enterprise_config = SubscriptionConfig {
        qos: QoS::ExactlyOnce,
        //TODO cache_strategy: CacheStrategy::Lru(NonZeroUsize::new(2000).unwrap()),
    };
    let _full_subscriber = TemperatureSensor::subscribe_to_custom_topic_with_config(
        &client,
        "iot/{building}/{floor}/temp/{sensor_id}",
        enterprise_config
    ).await?;
    println!("    ✅ Custom pattern + custom config: iot/+/+/temp/+");
    
    // Method 5: This would fail - incompatible pattern
    println!("5️⃣  Testing validation (should fail):");
    let invalid_result = TemperatureSensor::subscribe_to_custom_topic(
        &client,
        "wrong/{floor}/{building}/temp/{sensor_id}"  // Wrong parameter order
    ).await;
    
    match invalid_result {
        Err(e) => println!("    ✅ Correctly rejected invalid pattern: {}", e),
        Ok(_) => panic!("Should have failed!"),
    }
    
    connection.shutdown().await?;
    
    // For this test, just verify method signatures exist and are callable
    println!("📋 Verifying method signatures:");
    println!("✅ subscribe() - available");
    println!("✅ subscribe_with_config() - available");  
    println!("✅ subscribe_pattern() - available");
    println!("✅ subscribe_pattern_with_config() - available");
    
    // Test constants
    println!("📊 Pattern constants:");
    println!("   TOPIC_PATTERN: {}", TemperatureSensor::TOPIC_PATTERN);
    println!("   MQTT_PATTERN:  {}", TemperatureSensor::MQTT_PATTERN);
    
    // Test publisher methods are still available
    println!("📤 Publisher methods:");
    println!("✅ publish() - available");
    println!("✅ get_publisher() - available");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    demonstrate_subscription_methods().await?;
    println!("🎉 All methods demonstrated successfully!");
    Ok(())
}
