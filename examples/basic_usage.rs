//! Basic usage example for mqtt_typed_client
//!
//! This example demonstrates how to use the MQTT typed client library
//! to publish and subscribe to typed messages.
//!
//! Run with: cargo run --example basic_usage --features examples

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{MqttAsyncClient, BincodeSerializer};
use serde::{Deserialize, Serialize};
use tokio::time;

use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct MyData {
    id: u32,
}

pub async fn run_example() -> Result<(), Box<dyn std::error::Error>> {
    info!("Creating MQTT client");
    
    let client = MqttAsyncClient::<BincodeSerializer>::new(
        "mqtt://broker.mqtt.cool:1883?client_id=rumqtt-async-example",
    )
    .await?;
    
    info!("MQTT client created successfully");

    info!("Setting up publisher and subscriber");
    
    let publisher = client.get_publisher::<MyData>("hello/typed")?;
    let mut subscriber = client
        .subscribe::<MyData>("hello/typed")
        .await?;
    
    info!("Publisher and subscriber ready");

    // Spawn publisher task
    tokio::spawn(async move {
        for i in 0..10 {
            debug!(message_id = i, "Publishing message");

            let data = MyData { id: i };

            let res = publisher.publish(&data).await;
            match res {
                | Ok(()) => {
                    debug!(message_id = i, "Message published successfully")
                }
                | Err(err) => {
                    error!(message_id = i, error = %err, "Failed to publish message");
                    
                    eprintln!("Failed to publish message {}: {}", i, err);
                }
            }
            time::sleep(Duration::from_millis(500)).await;
        }
    });

    let mut client_opt = Some(client);
    let mut count = 0;
    
    info!("Starting message reception loop");

    while let Some((topic, data)) = subscriber.receive().await {
        if count == 5 {
            if let Some(client) = client_opt.take() {
                info!("Shutting down client after receiving 5 messages");
                
                let _res = client.shutdown().await;
                
                info!(result = ?_res, "Client shutdown completed");
            }
        }
        
        match data {
            Ok(data) => {
                info!(topic = %topic, data = ?data, count = count, "Received message");
                
                println!("Received from {}: {:?} (count: {})", topic, data, count);
            }
            Err(err) => {
                error!(topic = %topic, count = count, error = ?err, "Failed to deserialize message data");
                
                eprintln!("Failed to deserialize message from {}: {:?}", topic, err);
            }
        }
        count += 1;
    }
    
    info!("Exited from subscriber listen loop");
    
    time::sleep(Duration::from_secs(2)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber only if examples feature is enabled
    {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| {
                        "mqtt_typed_client=debug,rumqttc=info".into()
                    }),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_thread_names(false)
                    .with_file(false)
                    .with_line_number(false)
                    .compact(),
            )
            .init();

        info!("Starting MQTT typed client example");
    }

    let result = run_example().await;
    
    if let Err(ref e) = result {
        error!(error = %e, "Example failed");
        
        eprintln!("Example failed: {}", e);
    } else {
        info!("Example completed successfully");
        
        println!("Example completed successfully");
    }
    
    result
}
