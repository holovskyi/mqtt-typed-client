use std::{sync::Arc, time::Duration};

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, MessageSerializer, MqttClient, MqttClientError,
	MqttPublisher,
	topic::topic_match::TopicMatch,
	errors::TopicRouterError,
};
use tokio::time;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/* payload concrete type */
#[derive(Debug, PartialEq, Encode, Decode)]
struct SensorData {
	timestamp: u64,
	measure_id: u32,
	value: f32,
}

// Recursive expansion of mqtt_topic_subscriber macro
// ===================================================

#[derive(Debug)]
struct SensorReading {
	sensor_id: u32,
	room: String,
	temp: f32,
	payload: SensorData,
	topic: Arc<TopicMatch>,
}
impl<DE> ::mqtt_typed_client::FromMqttMessage<SensorData, DE>
	for SensorReading
{
	fn from_mqtt_message(
		topic: ::std::sync::Arc<
			::mqtt_typed_client::topic::topic_match::TopicMatch,
		>,
		payload: SensorData,
	) -> ::std::result::Result<
		Self,
		::mqtt_typed_client::MessageConversionError<DE>,
	> {
		let sensor_id = ::mqtt_typed_client::extract_topic_parameter(
			&topic,
			1usize,
			"sensor_id",
		)?;
		let room = ::mqtt_typed_client::extract_topic_parameter(
			&topic, 0usize, "room",
		)?;
		let temp = ::mqtt_typed_client::extract_topic_parameter(
			&topic, 2usize, "temp",
		)?;
		Ok(Self {
			sensor_id,
			room,
			temp,
			payload,
			topic,
		})
	}
}
impl SensorReading {
	pub const TOPIC_PATTERN: &'static str =
		"typed/{room}/{sensor_id}/some/{temp}";
	pub const MQTT_PATTERN: &'static str = "typed/+/+/some/+";

	pub async fn subscribe<F>(
		client: &::mqtt_typed_client::MqttClient<F>,
	) -> ::std::result::Result<
		::mqtt_typed_client::MqttTopicSubscriber<Self, SensorData, F>,
		::mqtt_typed_client::MqttClientError,
	>
	where F: ::std::default::Default
			+ ::std::clone::Clone
			+ ::std::marker::Send
			+ ::std::marker::Sync
			+ ::mqtt_typed_client::MessageSerializer<SensorData> {
		let subscriber =
			client.subscribe::<SensorData>(Self::MQTT_PATTERN).await?;
		Ok(::mqtt_typed_client::MqttTopicSubscriber::new(
			subscriber,
		))
	}

	//EMULATE PUBLISHING CODEGEN
	// if in pattern present anonymous fields + it's translated  to
	// positional parameters for example "typed/{room}/+/some/{temp}
	// will be translated to wildcard_1:&str
	// all + has &str type
	//
	// Also if named paramteter in pattern not have field in struct it
	// also has &str type (we can generate warning for this case)
	// # wildcard not allowed for publish interface
	pub async fn publish<F>(
		client: &MqttClient<F>,
		sensor_id: u32,
		room: &str,
		temp: f32,

		data: &SensorData,
	) -> Result<(), MqttClientError>
	where
		F: MessageSerializer<SensorData>,
	{
		let publisher =
			SensorReading::get_publisher(client, sensor_id, room, temp)?;
		publisher.publish(data).await
	}

	pub fn get_publisher<F>(
		client: &MqttClient<F>,
		sensor_id: u32,
		room: &str,
		temp: f32,
	) -> Result<MqttPublisher<SensorData, F>, TopicRouterError>
	where
		F: MessageSerializer<SensorData>,
	{
		let topic = format!("typed/{room}/{sensor_id}/some/{temp}");
		client.get_publisher::<SensorData>(&topic)
	}
}

pub async fn test_main() -> Result<(), Box<dyn std::error::Error>> {
	info!("Creating MQTT client");
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		"mqtt://broker.mqtt.cool:1883?client_id=rumqtt-async",
	)
	.await?;
	info!("MQTT client created successfully");

	info!("Setting up publisher and subscriber");
	let publisher =
		client.get_publisher::<SensorData>("typed/room52/37/some/36.6")?;

	let mut subscriber_structured = SensorReading::subscribe(&client).await?;

	tokio::spawn(async move {
		for i in 0 .. 1000 {
			debug!(message_id = i, "Publishing message");

			let data = SensorData {
				timestamp: 1633036800 + i as u64, // Example timestamp
				measure_id: i,
				value: 36.6 + (i as f32 * 0.1), // Example value
			};

			let res = publisher.publish(&data).await;
			match res {
				| Ok(()) => {
					debug!(message_id = i, "Message published successfully")
				}
				| Err(err) => {
					error!(message_id = i, error = %err, "Failed to publish message")
				}
			}
			time::sleep(Duration::from_secs(1)).await;
		}
	});
	let mut connection_opt = Some(connection);
	let mut count = 0;
	info!("Starting message reception loop");
	while let Some(sensor_result) = subscriber_structured.receive().await {
		//info!(topic = ?topic_match,  "Received message on topic");
		if count == 10 {
			// if let Err(err) = subscriber.cancel().await {
			//     warn!(error = %err, "Failed to cancel subscription");
			// }
			if let Some(connection) = connection_opt.take() {
				info!("Shutting down client after receiving 10 messages");
				let _res = connection.shutdown().await;
				info!(result = ?_res, "Client shutdown completed");
			}
			//break;
		}
		match sensor_result {
			| Ok(sensor_reading) => {
				info!(?sensor_reading, "Parsed sensor reading");
			}
			| Err(err) => {
				error!(error = %err, "Failed to receive data");
			}
		}
		count += 1;
	}
	info!("Exited from subscriber listen loop");
	//subscriber.cancel();
	time::sleep(Duration::from_secs(20)).await;
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing subscriber with compact formatting
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

	info!("Starting MQTT typed client application");
	let result = test_main().await;
	if let Err(ref e) = result {
		error!(error = %e, "Application failed");
	} else {
		info!("Application completed successfully");
	}
	result
}
