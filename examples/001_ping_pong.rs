//! # Ping Pong Game - MQTT Typed Client Example
//!
//! Demonstrates interactive bi-directional communication between two players
//! through MQTT topics using the typed client pattern.
//!
//! Key features showcased:
//! - Bi-directional messaging between multiple clients
//! - Game state management through MQTT
//! - Randomized game logic with probabilistic outcomes
//! - Concurrent player execution
//!
//! Topic pattern: "game/{player}"
//! - Alice subscribes to: "game/alice"
//! - Bob subscribes to: "game/bob"
//! - Publishing: client.publish("alice", message) → "game/alice"
//! - Receiving: "game/alice" → PingPongTopic { player: "alice", payload: deserialized_msg }
//!
//! Game flow:
//! 1. Both players subscribe to their respective topics
//! 2. Bob starts by sending Ping(0) to Alice
//! 3. Players alternate sending Ping/Pong messages with incrementing counters
//! 4. Each move has a 5% chance to end the game
//! 5. The player who sends GameOver loses, the other wins

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientError};
use mqtt_typed_client_macros::mqtt_topic;
use rand::Rng;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Message types for the ping pong game
///
/// The game alternates between Ping and Pong messages with incrementing counters.
/// GameOver signals the end of the game.
#[derive(Encode, Decode, Debug)]
enum PingPongMessage {
	Ping(usize), // Ping with move counter
	Pong(usize), // Pong with move counter
	GameOver,    // Game termination signal
}

impl PingPongMessage {
	/// Generate next move with 95% chance to continue, 5% chance to end game
	fn next_move(&self) -> PingPongMessage {
		if !rand::rng().random_bool(0.95) {
			return PingPongMessage::GameOver;
		}
		match self {
			| PingPongMessage::Ping(n) => PingPongMessage::Pong(n + 1),
			| PingPongMessage::Pong(n) => PingPongMessage::Ping(n + 1),
			| PingPongMessage::GameOver => PingPongMessage::GameOver,
		}
	}
	fn is_game_over(&self) -> bool {
		matches!(self, PingPongMessage::GameOver)
	}
}

/// MQTT topic structure for game communication
///
/// Pattern: "game/{player}"
/// Each player subscribes to their own topic and publishes to opponent's topic
#[derive(Debug)]
#[mqtt_topic("game/{player}")]
pub struct PingPongTopic {
	payload: PingPongMessage,
}

/// Get MQTT broker URL from environment or use default public broker
fn broker_url() -> String {
	std::env::var("MQTT_BROKER").unwrap_or_else(|_| {
		//"mqtt://broker.hivemq.com:1883?client_id=test_client_example".to_string()
		//You can try other free mqtt broker
		"mqtt://broker.mqtt.cool:1883?client_id=test_client_example".to_string()
	})
}

type MySerializer = BincodeSerializer;

/// Run a single player's game session
async fn run_player(
	client: MqttClient<MySerializer>,
	player: &str,
	other_player: &str,
	is_starter: bool,
) -> Result<(), MqttClientError> {
	// Get typed topic client for PingPongTopic
	let topic_client = client.ping_pong_topic();

	// Subscribe to this player's topic: "game/{player}"
	// .for_player() filters subscription to specific player ("game/alice")
	// Without .for_player() it would subscribe to "game/+" (all players)
	let mut subscriber = topic_client
		.subscription()
		.for_player(player)
		.subscribe()
		.await?;

	let ping_message = PingPongMessage::Ping(0);

	// Starter player sends first message to opponent
	if is_starter {
		topic_client.publish(other_player, &ping_message).await?;
	}

	// Main game loop: receive messages and respond
	while let Some(result) = subscriber.receive().await {
		match result {
			Ok(response) => {
				println!("{player:>10} received: {response:?}");

				if response.payload.is_game_over() {
					println!("{player:>10} Yarrr! I am the winner!");
					break;
				}

				let reply = response.payload.next_move();

				// Publish response to opponent's topic: "game/{other_player}"
				topic_client.publish(other_player, &reply).await?;

				if reply.is_game_over() {
					println!("{player:>10}: Ups... I'm lost...");
					break;
				}
			}
			Err(err) => {
				eprintln!("{player:>10} deserialization error: {err:?}");
				continue;
			}
		}
	}

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
	println!("Starting MQTT Ping Pong example...\n");

	// Connect to MQTT broker using BincodeSerializer for efficient binary serialization
	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&broker_url())
			.await
			.inspect_err(|e| {
				eprintln!("Connection failed: {e}");
				eprintln!(
					"Try: MQTT_BROKER=\"mqtt://localhost:1883\" cargo run \
					 --example 001_ping_pong"
				);
			})?;

	let client_clone = client.clone();
	let alice_handler =
		async move { run_player(client_clone, "alice", "bob", false).await };
	let bob_handler = async move {
	    // NOTE: This sleep is a simplification for demonstration purposes.
		// In production code, you should implement proper synchronization 
		// (e.g., discovery topic, heartbeat pattern, or message acknowledgment).
		// See advanced examples for robust synchronization techniques.

		// Give Alice time to subscribe
		tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await; 
		run_player(client, "bob", "alice", true).await
	};

	let _ = tokio::join!(alice_handler, bob_handler);

	connection.shutdown().await?;
	println!("\nGoodbye!");

	Ok(())
}
