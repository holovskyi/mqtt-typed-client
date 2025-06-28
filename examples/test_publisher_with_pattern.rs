//! Test to verify that PublisherBuilder actually uses its pattern
//!
//! This test verifies that PublisherBuilder.with_pattern() actually affects
//! the generated topic strings when using builder methods.

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
	println!("🧪 Testing PublisherBuilder with custom patterns");

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
	let default_publisher = SensorMessage::publisher();
	println!(
		"   Pattern: {}",
		default_publisher.pattern().topic_pattern()
	);

	// This should generate topic: "sensors/temp_001/data"
	match default_publisher.get_publisher(&client, sensor_id) {
		| Ok(_publisher) => {
			println!("   ✅ Default publisher created successfully")
		}
		| Err(e) => println!("   ❌ Error: {}", e),
	}

	// Test 2: Publisher with custom pattern
	println!("\n2️⃣  Testing publisher with custom pattern:");
	let custom_pattern = "devices/{sensor_id}/readings";
	let custom_publisher = SensorMessage::publisher()
		.with_pattern(custom_pattern)?
		.with_qos(QoS::AtLeastOnce);

	custom_publisher
		.publish(&client, sensor_id, &TestData { value: 42 })
		.await?;
	println!("   Pattern: {}", custom_publisher.pattern().topic_pattern());

	// This should generate topic: "devices/temp_001/readings"
	match custom_publisher.get_publisher(&client, sensor_id) {
		| Ok(_publisher) => {
			let real_topic = _publisher.topic();
			println!("   Publisher topic: {}", real_topic);
			assert_eq!(real_topic, "devices/temp_001/readings");
			_publisher.publish(&TestData { value: 42 }).await?;
			println!("   ✅ Custom publisher created successfully")
		}
		| Err(e) => println!("   ❌ Error: {}", e),
	}

	// Test 3: Publisher with incompatible pattern (should fail)
	println!(
		"\n3️⃣  Testing publisher with incompatible pattern (should fail):"
	);
	match SensorMessage::publisher()
		.with_pattern("wrong/pattern/without/wildcards")
	{
		| Ok(_) => println!("   ❌ Should have failed!"),
		| Err(e) => println!("   ✅ Correctly rejected: {}", e),
	}

	// Test 4: Verify that patterns actually affect the generated topics
	println!("\n4️⃣  Verifying topic generation:");

	// We can't easily test the actual topic without mocking, but we can verify
	// that different patterns are stored correctly
	let pattern1 = SensorMessage::publisher().pattern().topic_pattern();
	let pattern2 = SensorMessage::publisher()
		.with_pattern("iot/{sensor_id}/telemetry")?
		.pattern()
		.topic_pattern();

	println!("   Default pattern: {}", pattern1);
	println!("   Custom pattern:  {}", pattern2);

	if pattern1 != pattern2 {
		println!("   ✅ Patterns are different - pattern substitution works!");
	} else {
		println!("   ❌ Patterns are the same - something is wrong!");
	}

	connection.shutdown().await?;

	println!("\n🎉 PublisherBuilder pattern test completed!");
	Ok(())
}
