use mqtt_typed_client_macros::mqtt_topic_subscriber;
use mqtt_typed_client::message_serializer::BincodeSerializer;
use mqtt_typed_client::client::async_client::MqttClient;
use std::num::NonZeroUsize;
use std::sync::Arc;

#[derive(Debug)]
#[mqtt_topic_subscriber("sensors/{sensor_id}/temperature")]
struct TemperatureReading {
    sensor_id: u32,
    payload: Vec<u8>,
    topic: Arc<mqtt_typed_client::topic::topic_match::TopicMatch>,
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing full MQTT macro integration...");
    
    // Показуємо інформацію про згенерований код
    println!("📋 Topic pattern: {}", TemperatureReading::TOPIC_PATTERN);
    
    // Створюємо структуру

    
    // Тепер спробуємо підключитися до MQTT (це може не працювати без справжнього брокера)
    println!("🔌 Attempting MQTT connection...");
    
    // УВАГА: Це потребує справжнього MQTT брокера!
    // Поки що тільки показуємо що метод існує
    match MqttClient::<BincodeSerializer>::new(
        "mqtt://broker.mqtt.cool:1883?client_id=test_client", 
        NonZeroUsize::new(100).unwrap(),
        10,
        10
    ).await {
        Ok((client, _connection)) => {
            println!("✅ MQTT client created successfully!");
            
            // Тепер можемо спробувати підписатися
            match TemperatureReading::subscribe(&client).await {
                Ok(subscriber) => {
                    println!("🎉 Successfully subscribed using macro-generated method!");
                    // subscriber тепер готовий отримувати повідомлення
                }
                Err(e) => {
                    println!("❌ Subscription failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ MQTT connection failed (expected without broker): {}", e);
            println!("✅ But macro compilation successful!");
        }
    }
    
    Ok(())
}