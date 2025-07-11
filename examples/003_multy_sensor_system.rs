//! # TODO

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, MqttClient, MqttClientConfig, MqttClientError,
};
use mqtt_typed_client_macros::mqtt_topic;
use uuid::Uuid;

#[derive(Encode, Decode, Debug)]
struct SensorData {
	temperature: f32,
}

#[derive(Debug)]
#[mqtt_topic("sensors/{location}/{sensor_guid}")]
pub struct SensorTopic {
	location: String,
	sensor_guid: Uuid,
	payload: SensorData,
}

async fn run_sensor_emulator(
	client: MqttClient<BincodeSerializer>,
	location: &str,
) -> Result<(), MqttClientError> {
	let sensor_guid = Uuid::new_v4(); // Generate a unique sensor GUID

	let sensor_client = client.sensor_topic();

	// Create and configure the sensor publisher
	// This will publish to the topic "sensors/{location}/{sensor_guid}"
	// where {location} is the location of the sensor and {sensor_guid} is the unique identifier
	// for the sensor.
	// The topic will look like "sensors/living_room/123e4567-e89b-12d3-a456-426614174000"
	// for a sensor in the living room with a specific GUID.
	let sensor_publisher = sensor_client
		.get_publisher(location, sensor_guid)?
		.with_qos(rumqttc::QoS::AtMostOnce)
		.with_retain(true);

	// Simulate sensor data publishing
	loop {
		let sensor_data = SensorData {
			temperature: rand::random::<f32>() * 100.0, // Random temperature
		};

		sensor_publisher.publish(&sensor_data).await?;

		tokio::time::sleep(Duration::from_secs(5)).await; // Publish every 5 seconds
	}
}

async fn run_all_sensor_monitor(
	client: MqttClient<BincodeSerializer>,
) -> Result<(), MqttClientError> {
	let sensor_client = client.sensor_topic();

	// Configure the sensor subscription
	// This will subscribe to all sensor topics in the format "sensors/{location}/{sensor
	let mut sensor_subscription = sensor_client
		.subscription()
		.with_qos(rumqttc::QoS::AtMostOnce)
		.with_cache(100)
		.subscribe()
		.await?;

	// Process incoming messages
	while let Some(Ok(sensor_topic)) = sensor_subscription.receive().await {
		println!(
			"Sensor data received: Location: {}, Sensor GUID: {}, \
			 Temperature: {}°C",
			sensor_topic.location,
			sensor_topic.sensor_guid,
			sensor_topic.payload.temperature
		);
	}
	println!("Sensor monitor stopped.");
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = MqttClientConfig::<BincodeSerializer>::new(
		"test_client",
		"broker.mqtt.cool",
		1883,
	);

	// Configure rumqttc connection options - MqttOptions
	// Set keep-alive interval, clean session, and etc.
	config.connection
		.set_keep_alive(Duration::from_secs(60))
		.set_clean_session(false)
		//.set_credentials("username", "password")
		;

	// Configure ClientSettings
	// Fine tune high-level client settings
	config.settings.event_loop_capacity = 100;
	config.settings.command_channel_capacity = 100;
	config.settings.unsubscribe_channel_capacity = 10;
	config.settings.connection_timeout_millis = 5000;
	config.settings.topic_cache_size = 100;

	let (client, connection) = MqttClient::connect_with_config(config)
		.await
		.inspect_err(|e| {
			eprintln!("Connection failed: {e}");
		})?;

	let locations = ["living_room", "kitchen", "bedroom"];
	let sensor_handles: Vec<_> = locations
		.iter()
		.map(|location| {
			let client_clone = client.clone();
			let location = location.to_string();
			tokio::spawn(async move {
				let res = run_sensor_emulator(client_clone, &location).await;
				println!(
					"Sensor emulator for {location} finished with result: \
					 {res:?}"
				);
				res
			})
		})
		.collect();

	let monitor_handle = {
		let client_clone = client.clone();
		tokio::spawn(async move { run_all_sensor_monitor(client_clone).await })
	};

	tokio::signal::ctrl_c().await?;
	println!("\nCtrl+C received, shutting down...");
	connection.shutdown().await?;

	for handle in sensor_handles {
		let _ = handle.await;
	}
	let _ = monitor_handle.await;

	println!("\nGoodbye!");
	Ok(())
}
