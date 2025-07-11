//! Advanced configuration example for mqtt_typed_client
//!
//! This example demonstrates how to use the advanced configuration options
//! including TLS, credentials, custom timeouts, and performance tuning.
//!
//! Run with: cargo run --example advanced_config --features examples

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientConfig, ClientSettings};
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct SensorData {
    sensor_id: u32,
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

pub async fn run_example() -> Result<(), Box<dyn std::error::Error>> {
    info!("Demonstrating advanced MQTT client configuration");

    // Example 1: Basic configuration with custom settings
    let mut config = MqttClientConfig::new("advanced_client", "broker.mqtt.cool", 1883);
    
    // Configure MQTT connection options
    config.connection.set_keep_alive(Duration::from_secs(30));
    config.connection.set_clean_session(true);
    config.connection.set_max_packet_size(1024 * 1024, 1024 * 1024); // 1MB
    
    // Configure client performance settings
    config.settings.topic_cache_size = 500;
    config.settings.event_loop_capacity = 50;
    config.settings.command_channel_capacity = 200;
    
    info!("Creating MQTT client with advanced configuration");
    let (client, connection) = MqttClient::<BincodeSerializer>::connect_with_config(config).await?;
    
    // Example 2: Localhost development configuration
    let _dev_config = MqttClientConfig::<BincodeSerializer>::localhost("dev_client");
    
    // Example 3: URL-based configuration with query parameters
    let _url_config = MqttClientConfig::<BincodeSerializer>::from_url(
        "mqtt://broker.mqtt.cool:1883?client_id=url_client&keep_alive=60"
    )?;
    
    // Example 4: Configuration for high-throughput applications
    let mut high_perf_config = MqttClientConfig::<BincodeSerializer>::new("high_perf", "broker.mqtt.cool", 1883);
    high_perf_config.connection.set_inflight(100); // Increase concurrent messages
    high_perf_config.connection.set_request_channel_capacity(1000);
    high_perf_config.settings = ClientSettings {
        topic_cache_size: 1000,
        event_loop_capacity: 100,
        command_channel_capacity: 500,
        unsubscribe_channel_capacity: 50,
        connection_timeout_millis: 5000, // 5 seconds
    };
    
    info!("Setting up publisher and subscriber");
    let publisher = client.get_publisher::<SensorData>("sensors/temperature/room1")?;
    let mut subscriber = client.subscribe::<SensorData>("sensors/+/+").await?;
    
    // Publish some test data
    tokio::spawn(async move {
        for i in 0..5 {
            let data = SensorData {
                sensor_id: i,
                temperature: 20.0 + (i as f64 * 0.5),
                humidity: 45.0 + (i as f64),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            if let Err(e) = publisher.publish(&data).await {
                error!(error = %e, "Failed to publish sensor data");
            } else {
                info!(sensor_id = i, "Published sensor data");
            }
            
            time::sleep(Duration::from_secs(1)).await;
        }
    });
    
    // Receive messages
    let mut count = 0;
    while let Some((topic, result)) = subscriber.receive().await {
        match result {
            Ok(data) => {
                info!(
                    topic = %topic,
                    sensor_id = data.sensor_id,
                    temperature = data.temperature,
                    humidity = data.humidity,
                    "Received sensor data"
                );
                
                count += 1;
                if count >= 3 {
                    info!("Received enough messages, shutting down");
                    break;
                }
            }
            Err(e) => {
                error!(topic = %topic, error = ?e, "Failed to deserialize sensor data");
            }
        }
    }
    
    // Graceful shutdown
    connection.shutdown().await?;
    info!("Advanced configuration example completed successfully");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| "debug".into()),
    //     )
    //     .with(
    //         tracing_subscriber::fmt::layer()
    //             .with_target(true)
    //             .with_thread_ids(false)
    //             .with_thread_names(false)
    //             .with_file(false)
    //             .with_line_number(false)
    //             .compact(),
    //     )
    //     .init();
	tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "debug".into()),
		)
		.with(
			tracing_subscriber::fmt::layer()
				.with_target(true) // Hide module target for cleaner output
				.with_thread_ids(false) // Hide thread IDs
				.with_thread_names(false) // Hide thread names
				.with_file(false) // Hide file info
				.with_line_number(false) // Hide line numbers
				.compact(), // More compact output
		)
		.init();
    info!("Starting advanced MQTT configuration example");
    
    let result = run_example().await;
    
    if let Err(ref e) = result {
        error!(error = %e, "Example failed");
        eprintln!("Example failed: {e}");
    } else {
        info!("Example completed successfully");
        println!("Advanced configuration example completed successfully!");
    }
    
    result
}
