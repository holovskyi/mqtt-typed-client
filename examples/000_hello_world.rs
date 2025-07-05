//! # Philosophical Dialogue - MQTT Typed Client Example
//!
//! Demonstrates pattern-based MQTT routing through a conversation between
//! various subjects and the World using topic pattern "universum/{to}/{from}".
//!
//! This example showcases how to use the MQTT Typed Client to create a
//! structured communication system where different actors can ask the World
//!
use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientConfig};
use mqtt_typed_client_macros::mqtt_topic;
use tokio::signal;
use tracing::info;

#[derive(Encode, Decode)]
struct Message {
	text: String,
}

// Pattern transformations:
// Subscription: .for_to("World") → "universum/World/+"
//               .for_from("Cat")  → "universum/+/Cat"
//               no filters        → "universum/+/+"
// Publishing:   .publish("World", "Rustacean", msg) → "universum/World/Rustacean"
// Receiving:    "universum/World/Rustacean" → MessageTopic { to: "World", from: "Rustacean", payload: deserialized_msg }
#[mqtt_topic("universum/{to}/{from}")]
pub struct MessageTopic {
	//to: String, // field may be omitted if not used
	from: String,
	payload: Message,
}

const WORLD: &str = "World";

async fn spawn_world(
	client: &MqttClient<BincodeSerializer>,
) -> mqtt_typed_client::Result<()> {
	println!("\nUniversum spawned!\n");
	let topic_client = client.message_topic();

	let mut subscriber = topic_client
		.subscription()
		.for_to(WORLD)
		.subscribe()
		.await?;

	while let Some(Ok(received_message)) = subscriber.receive().await {
		let actor = &received_message.from;
		let answer = get_world_response(actor);
		topic_client.publish(actor, WORLD, &answer).await?;
	}
	// Waiting for all actors to leave the chat
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
	println!("\nUniversum collapsed!\n");
	Ok(())
}

async fn spawn_actor(
	client: &MqttClient<BincodeSerializer>,
	actor: &str,
	question: &str,
) -> mqtt_typed_client::Result<()> {
	let topic_client = client.message_topic();

	let mut subscriber = topic_client
		.subscription()
		.for_to(actor)
		.for_from(WORLD)
		.subscribe()
		.await?;

	let publisher = topic_client.get_publisher(WORLD, actor)?;

	let question_msg = Message {
		text: question.to_string(),
	};

	publisher.publish(&question_msg).await?;
	if let Some(Ok(message)) = subscriber.receive().await {
		println!("{:>12}: {}", actor, question_msg.text);
		println!("{:>12}: {}\n", message.from, message.payload.text);

		while let Some(Ok(message)) = subscriber.receive().await {
			println!("{:>12}: {}\n", message.from, message.payload.text);
		}
	} else {
		println!("{actor} did not receive a response from the World.");
	}

	println!("{actor} left the chat");
	Ok(())
}

fn get_broker_url() -> String {
	std::env::var("MQTT_BROKER").unwrap_or_else(|_| {
		"mqtt://broker.hivemq.com:1883?client_id=hello_world_example"
			//"mqtt://broker.mqtt.cool:1883?client_id=hello_world_example"
			.to_string()
	})
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
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

	let mut config = MqttClientConfig::from_url(&get_broker_url())?;

	// Configure MQTT connection options
	config.connection.set_keep_alive(Duration::from_secs(30));
    info!("Clean session = {}", config.connection.clean_session());
    config.connection.set_clean_session(false);

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect_with_config(config)
			.await
			.inspect_err(|_| {
				eprintln!("❌ Connection failed. Try:");
				eprintln!(
					"  MQTT_BROKER=\"mqtt://localhost:1883\" cargo run \
					 --example 000_hello_world"
				);
			})?;
	// let (client, connection) =
	// 	MqttClient::<BincodeSerializer>::connect(&get_broker_url())
	// 		.await
	// 		.map_err(|e| {
	// 			eprintln!("❌ Connection failed. Try:");
	// 			eprintln!(
	// 				"  MQTT_BROKER=\"mqtt://localhost:1883\" cargo run \
	// 				 --example 000_hello_world"
	// 			);
	// 			e
	// 		})?;
	let client_clone = client.clone();
	tokio::spawn(async move {
		if let Err(e) = spawn_world(&client_clone).await {
			eprintln!("Error in world: {e}");
		}
	});

	for conversation in CONVERSATIONS {
		let client_clone = client.clone();
		// Delay to allow the world to spawn before starting conversations
		// and waiting while previous conversations are spawned
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
		tokio::spawn(async move {
			if let Err(e) = spawn_actor(
				&client_clone,
				conversation.name,
				conversation.question,
			)
			.await
			{
				eprintln!("Error in actor {}: {e}", conversation.name);
			}
		});
	}

	tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
	println!("All questions asked! Press Ctrl+C to collapse the universe...");
	signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
	println!("\nReturning to the void... It was fun while it lasted!\n");

	connection.shutdown().await?;

	// Wait until all actors have left the chat and the world has collapsed
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
	Ok(())
}

struct ActorConversation {
	name: &'static str,
	question: &'static str,
	answer: &'static str,
}

fn get_world_response(actor: &str) -> Message {
	let text = CONVERSATIONS
		.iter()
		.find(|conversation| conversation.name == actor)
		.map(|ActorConversation { name, answer, .. }| {
			format!("Hello {name}! {answer}")
		})
		.unwrap_or_else(|| "Go in peace, unknown wanderer".to_string());
	Message { text }
}

const CONVERSATIONS: &[ActorConversation] = &[
	ActorConversation {
		name: "Rustacean",
		question: "println!(\"Hello World!\");",
		answer: "Hello World!",
	},
	ActorConversation {
		name: "Philosopher",
		question: "Hello World! What is your essence?",
		answer: "I am what you perceive.",
	},
	ActorConversation {
		name: "Skeptic",
		question: "Hello World! Do you really exist?",
		answer: "Does it matter?",
	},
	ActorConversation {
		name: "Mystic",
		question: "Hello World! I feel your presence.",
		answer: "We are one.",
	},
	ActorConversation {
		name: "Scientist",
		question: "Hello World! I will measure and test you.",
		answer: "I reveal my secrets through patterns.",
	},
	ActorConversation {
		name: "Cat",
		question: "Meow!",
		answer: "You understand without questions.",
	},
];
