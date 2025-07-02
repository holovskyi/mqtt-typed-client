use std::{sync::Arc, time::Duration};

use bincode::{Decode, Encode};
use mqtt_typed_client::{
	BincodeSerializer, MqttClient, topic::topic_match::TopicMatch,
};
//extern crate mqtt_typed_client_macros;
use mqtt_typed_client_macros::mqtt_topic;
// Extension trait буде згенерований макросом
//use mqtt_async_client::MqttAsyncClient;
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/* payload concrete type */
#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct SensorData {
	timestamp: u64,
	measure_id: u32,
	value: f32,
}

/* MQTT message with all data */
#[allow(dead_code)]
#[mqtt_topic("typed/{room}/pl/{sensor_id}/some/{temp}")]
#[derive(Debug)]
struct SensorReading {
	sensor_id: u32, // extracted from topic field. Other fiellds not allowed here
	room: String,
	temp: f32,

	payload: SensorData,    // optional filed
	topic: Arc<TopicMatch>, // optional filed
}

fn get_server(server: &str, client_id: &str) -> String {
	format!("{server}?client_id={client_id}&clean_session=true")
}
const _SERVER_COOL: &str = "mqtt://broker.mqtt.cool:1883";
const _SERVER_MODSQITO: &str = "mqtt://test.mosquitto.org:1883";
const SERVER: &str = _SERVER_COOL;

async fn run_publisher() -> Result<(), Box<dyn std::error::Error>> {
	info!("Creating MQTT client");
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		&get_server(SERVER, "rust-publisher"),
	)
	.await?;
	info!("MQTT client created successfully");

	info!("Setting up publisher and subscriber");

	let sensor_client = client.sensor_reading();

	let publisher = sensor_client.get_publisher("room52", 37, 36.6)?;

	for i in 0 .. 10 {
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
	info!("Publisher finished sending messages");
	let _res = connection.shutdown().await;
	info!(result = ?_res, "Client shutdown completed");
	Ok(())
}

async fn run_subscriber() -> Result<(), Box<dyn std::error::Error>> {
	info!("Creating MQTT client");
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		&get_server(SERVER, "rust-subscriber"),
	)
	.await?;
	info!("MQTT client created successfully");

	info!(mqtt = SensorReading::MQTT_PATTERN, "Setting up subscriber");

	let sensor_client = client.sensor_reading();

	
	// // Демонстрація фільтрованої підписки
	let mut subscriber = sensor_client
		.subscription()
		.for_room("room52")  // Фільтр для конкретної кімнати
		
		//.for_sensor_id(37)   // Фільтр для конкретного сенсора  
		.with_qos(rumqttc::QoS::AtLeastOnce)
		.subscribe()
		.await?;

	// info!("Створено фільтровану підписку на room52/sensor37");

	// Основна підписка (на всі повідомлення)
	//let mut subscriber = sensor_client.subscribe().await?;

	//TODO show using without macro SensorReading::subscribe(&client).await?;

	let mut count = 0;
	info!("Starting message reception loop with filtered subscriber");
	while let Some(sensor_result) = 	subscriber.receive().await {
		if count == 10 {
			break;
		}
		match sensor_result {
			| Ok(sensor_reading) => {
				info!(?sensor_reading, "Filtered sensor result received (room52/sensor37)");
			}
			| Err(err) => {
				error!(error = %err, "Failed to receive filtered data");
			}
		}
		count += 1;
	}

	info!("Subscriber finished");
	let _res = connection.shutdown().await;
	info!(result = ?_res, "Client shutdown completed");
	Ok(())
}

pub async fn test_main() -> Result<(), Box<dyn std::error::Error>> {
	info!("Creating MQTT client");
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		&get_server(SERVER, "rust-pub-sub"),
	)
	.await?;
	info!("MQTT client created successfully");

	info!("Setting up publisher and subscriber");
	let publisher = client
		.get_publisher::<SensorData>("typed/room52/other_plus/37/some/36.6")?;

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
	let args: Vec<String> = std::env::args().collect();

	if args.len() < 2 {
		println!("Usage: {} <mode>", args[0]);
		println!("Modes:");
		println!("  publisher  - Run as publisher only");
		println!("  subscriber - Run as subscriber only");
		println!(
			"  both       - Run both publisher and subscriber (original \
			 behavior)"
		);
		return Ok(());
	}
	let mode = &args[1];
	let result = match mode.as_str() {
		| "publisher" => {
			info!("Starting MQTT publisher");
			run_publisher().await
		}
		| "subscriber" => {
			info!("Starting MQTT subscriber");
			run_subscriber().await
		}
		| "both" => {
			info!("Starting MQTT typed client application (both modes)");
			test_main().await
		}
		| _ => {
			error!(
				"Invalid mode: {}. Use 'publisher', 'subscriber', or 'both'",
				mode
			);
			return Ok(());
		}
	};

	if let Err(ref e) = result {
		error!(error = %e, "Application failed");
	} else {
		info!("Application completed successfully");
	}
	result
}
