mod shared;

use std::time::Duration;
use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient};
use mqtt_typed_client_macros::mqtt_topic;

#[derive(Encode, Decode, Debug, Clone)]
struct DemoMessage {
    sequence: u32,
    content: String,
    sent_at: String,
    is_retained: bool,
}

#[mqtt_topic("demo/retain")]
pub struct RetainDemoTopic {
    payload: DemoMessage,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    shared::tracing::setup(None);

    println!("=== MQTT Retain & Clear Demo ===\n");

    tokio::join!(
        publisher_task(),
        supervisor_subscriber(),
        delayed_subscriber(1, "subscriber-1"),
        delayed_subscriber(6, "subscriber-2"),
        delayed_subscriber(11, "subscriber-3"),
        delayed_subscriber(16, "subscriber-4"),
    );

    Ok(())
}

async fn publisher_task() {
    if let Err(e) = run_publisher().await {
        eprintln!("Publisher error: {}", e);
    }
}

async fn run_publisher() -> Result<(), Box<dyn std::error::Error>> {
    let connection_url = shared::config::build_url("retain_publisher");
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(&connection_url).await?;
    
    let topic_client = client.retain_demo_topic();

    let msg1 = DemoMessage {
        sequence: 1,
        content: "First retained message".to_string(),
        sent_at: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
        is_retained: true,
    };
    
    println!("[PUBLISHER] t=0s: Publishing retained message #1");
    let publisher1 = topic_client.get_publisher()?.with_retain(true);
    publisher1.publish(&msg1).await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    let msg2 = DemoMessage {
        sequence: 2,
        content: "Updated retained message".to_string(),
        sent_at: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
        is_retained: true,
    };
    
    println!("[PUBLISHER] t=5s: Publishing retained message #2");
    let publisher2 = topic_client.get_publisher()?.with_retain(true);
    publisher2.publish(&msg2).await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    let msg3 = DemoMessage {
        sequence: 3,
        content: "Temporary message".to_string(),
        sent_at: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
        is_retained: false,
    };
    
    println!("[PUBLISHER] t=10s: Publishing non-retained message #3");
    let publisher3 = topic_client.get_publisher()?.with_retain(false);
    publisher3.publish(&msg3).await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    println!("[PUBLISHER] t=15s: Clearing retained message");
    let publisher4 = topic_client.get_publisher()?;
    publisher4.clear_retained().await?;

    tokio::time::sleep(Duration::from_secs(5)).await;
    println!("[PUBLISHER] Demo completed");

    connection.shutdown().await?;
    Ok(())
}

async fn supervisor_subscriber() {
    if let Err(e) = run_supervisor().await {
        eprintln!("Supervisor error: {}", e);
    }
}

async fn run_supervisor() -> Result<(), Box<dyn std::error::Error>> {
    let connection_url = shared::config::build_url("retain_supervisor");
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(&connection_url).await?;
    
    let topic_client = client.retain_demo_topic();
    let mut subscriber = topic_client.subscribe().await?;

    println!("[SUPERVISOR] Started continuous monitoring\n");

    // Run for demo duration then shutdown
    let _result = tokio::time::timeout(Duration::from_secs(25), async {
        while let Some(result) = subscriber.receive().await {
            match result {
                Ok(msg) => {
                    let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
                    println!("[SUPERVISOR] {}: Received seq={}, content='{}', sent_at={}, retained={}",
                        now, msg.payload.sequence, msg.payload.content, msg.payload.sent_at, msg.payload.is_retained);
                }
                Err(e) => {
                    eprintln!("[SUPERVISOR] Error: {}", e);
                }
            }
        }
    }).await;

    connection.shutdown().await?;
    Ok(())
}

async fn delayed_subscriber(delay_seconds: u64, client_id: &str) {
    if let Err(e) = run_delayed_subscriber(delay_seconds, client_id).await {
        eprintln!("[{}] Error: {}", client_id.to_uppercase(), e);
    }
}

async fn run_delayed_subscriber(delay_seconds: u64, client_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(Duration::from_secs(delay_seconds)).await;

    let connection_url = shared::config::build_url(&format!("retain_{}", client_id));
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(&connection_url).await?;
    
    let topic_client = client.retain_demo_topic();
    let mut subscriber = topic_client.subscribe().await?;

    let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
    println!("[{}] {}: Connected, waiting for retained message...", client_id.to_uppercase(), now);

    let timeout = tokio::time::timeout(Duration::from_secs(3), subscriber.receive()).await;
    
    match timeout {
        Ok(Some(Ok(msg))) => {
            let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
            println!("[{}] {}: Received retained message: seq={}, content='{}'", 
                client_id.to_uppercase(), now, msg.payload.sequence, msg.payload.content);
        }
        Ok(Some(Err(e))) => {
            println!("[{}] Error receiving message: {}", client_id.to_uppercase(), e);
        }
        Ok(None) | Err(_) => {
            let now = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
            println!("[{}] {}: No retained message available", client_id.to_uppercase(), now);
        }
    }

    connection.shutdown().await?;
    Ok(())

}
