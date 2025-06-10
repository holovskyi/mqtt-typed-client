use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient};
//use mqtt_async_client::MqttAsyncClient;
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct MyData {
	id: u32,
}

pub async fn test_main() -> Result<(), Box<dyn std::error::Error>> {
	info!("Creating MQTT client");
	let (client, connection) = MqttClient::<BincodeSerializer>::new(
		"mqtt://broker.mqtt.cool:1883?client_id=rumqtt-async",
	)
	.await?;
	info!("MQTT client created successfully");

	info!("Setting up publisher and subscriber");
	let publisher = client.get_publisher::<MyData>("hello/typed")?;
	let mut subscriber = client.subscribe::<MyData>("hello/typed").await?;
	info!("Publisher and subscriber ready");

	tokio::spawn(async move {
		for i in 0 .. 1000 {
			debug!(message_id = i, "Publishing message");

			let data = MyData { id: i };

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
	while let Some((topic, data)) = subscriber.receive().await {
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
		if let Ok(data) = data {
			info!(topic = %topic, data = ?data, count = count, "Received message");
		} else {
			error!(topic = %topic, count = count, "Failed to deserialize message data");
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
