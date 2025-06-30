//! Test to verify that get_publisher_to() method works with custom patterns
//!
//! This test verifies that the new simplified API correctly handles
//! custom topic patterns and validates pattern compatibility.

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient};
use mqtt_typed_client_macros::mqtt_topic;
use rumqttc::QoS;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct TestData {
	value: i32,
}

#[allow(dead_code)]
#[mqtt_topic("sensors/{sensor_id}/data", publisher)]
#[derive(Debug)]
struct SensorMessage {
	sensor_id: String,
	payload: TestData,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🧪 Testing get_publisher_to() with custom patterns");

	// This would work with a real MQTT broker:
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		"mqtt://broker.hivemq.com:1883?client_id=test_publisher_pattern",
	)
	.await?;

	let _test_data = TestData { value: 42 };
	let sensor_id = "temp_001";

	println!("📊 Default pattern: {}", SensorMessage::TOPIC_PATTERN);
	println!("📊 MQTT pattern: {}", SensorMessage::MQTT_PATTERN);

	// Test 1: Default publisher (uses default pattern)
	println!("\n1️⃣  Testing default publisher:");

	// This should generate topic: "sensors/temp_001/data"
	match SensorMessage::get_publisher(&client, sensor_id) {
		| Ok(publisher) => {
			let topic = publisher.topic();
			println!("   Generated topic: {topic}");
			assert_eq!(topic, "sensors/temp_001/data");
			publisher.publish(&_test_data).await?;
			println!("   ✅ Default publisher created successfully")
		}
		| Err(e) => println!("   ❌ Error: {e}"),
	}

	// Test 2: Publisher with custom pattern
	println!("\n2️⃣  Testing publisher with custom pattern:");
	let custom_pattern = "devices/{sensor_id}/readings";

	// This should generate topic: "devices/temp_001/readings"
	match SensorMessage::get_publisher_to(&client, custom_pattern, sensor_id) {
		| Ok(publisher) => {
			let topic = publisher.topic();
			println!("   Generated topic: {topic}");
			assert_eq!(topic, "devices/temp_001/readings");
			
			// Test with QoS configuration
			let configured_publisher = publisher.with_qos(QoS::AtLeastOnce);
			configured_publisher.publish(&_test_data).await?;
			println!("   ✅ Custom publisher created successfully")
		}
		| Err(e) => println!("   ❌ Error: {e}"),
	}

	// Test 3: Publisher with incompatible pattern (should fail)
	println!(
		"\n3️⃣  Testing publisher with incompatible pattern (should fail):"
	);
	match SensorMessage::get_publisher_to(&client, "wrong/pattern/without/wildcards", sensor_id)
	{
		| Ok(_) => println!("   ❌ Should have failed!"),
		| Err(e) => println!("   ✅ Correctly rejected: {e}"),
	}

	// Test 4: Verify that different patterns generate different topics
	println!("\n4️⃣  Verifying topic generation:");

	// Test with another custom pattern
	let iot_pattern = "iot/{sensor_id}/telemetry";
	match SensorMessage::get_publisher_to(&client, iot_pattern, sensor_id) {
		| Ok(publisher) => {
			let topic = publisher.topic();
			println!("   IoT pattern topic: {topic}");
			assert_eq!(topic, "iot/temp_001/telemetry");
			println!("   ✅ Different patterns generate different topics!");
		}
		| Err(e) => println!("   ❌ Error: {e}"),
	}

	// Test 5: Direct publish methods (no intermediate publisher)
	println!("\n5️⃣  Testing direct publish methods:");

	// Direct publish with default pattern
	println!("   📤 Publishing directly with default pattern...");
	match SensorMessage::publish(&client, sensor_id, &_test_data).await {
		| Ok(()) => println!("   ✅ Direct publish with default pattern successful"),
		| Err(e) => println!("   ❌ Error: {e}"),
	}

	// Direct publish to custom pattern (if we had such method)
	// Note: This would require a publish_to() method similar to get_publisher_to()
	println!("   📤 One-shot publishing to custom topic...");
	let custom_topic = "alerts/temp_001/high_temp";
	match client.get_publisher::<TestData>(custom_topic) {
		| Ok(publisher) => {
			publisher.publish(&TestData { value: 99 }).await?;
			println!("   ✅ One-shot publish to custom topic: {custom_topic}");
		}
		| Err(e) => println!("   ❌ Error: {e}"),
	}

	connection.shutdown().await?;

	println!("\n🎉 get_publisher_to() pattern test completed!");
	Ok(())
}
