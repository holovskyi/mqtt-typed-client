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

mod shared;

use std::time::Duration;

use bincode::{Decode, Encode};
use mqtt_typed_client::{BincodeSerializer, MqttClient, MqttClientError};
use mqtt_typed_client_macros::mqtt_topic;
use rand::{Rng, rng};

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
	fn next_move(&self) -> PingPongMessage {
		// Generate next move with 95% chance to continue, 5% chance to end game
		if rng().random_bool(0.05) {
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


/// Handle player's game session
async fn run_player(
	client: MqttClient<BincodeSerializer>,
	player: &str,
	other_player: &str,
	is_starter: bool,
) -> Result<(), MqttClientError> {
	// Get typed topic client for PingPongTopic
	let topic_client = client.ping_pong_topic();

	// Subscribe to this player's topic: "game/{player}"
	// .for_player("alice") subscribes to "game/alice" only
	// Without .for_player() subscribes to "game/+" (all players)
	let mut subscriber = topic_client
		.subscription()
		.for_player(player)
		.subscribe()
		.await?;

	// Starter player sends first message to opponent
	if is_starter {
		let ping_message = PingPongMessage::Ping(0);
		println!("{player:>10}: starts the game with {ping_message:?}\n");
		topic_client.publish(other_player, &ping_message).await?;
	}

	// Main game loop: receive messages and respond
	while let Some(result) = subscriber.receive().await {
		match result {
			| Ok(response) => {
				println!("{player:>10} received: {:?}", response.payload);

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
			| Err(err) => {
				eprintln!("{player:>10} deserialization error: {err:?}");
				continue;
			}
		}
	}

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	println!("Starting MQTT Ping Pong example...\n");

	// === 1. CONNECTION ===
	// Connect to MQTT broker using BincodeSerializer for efficient binary serialization
	// URL and client_id are automatically configured from environment or defaults
	let connection_url = shared::config::build_url("ping_pong");
	println!("Connecting to MQTT broker: {connection_url}");
	
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(&connection_url)
		.await
		.inspect_err(|e| {
			shared::config::print_connection_error(&connection_url, e);
		})?;

	println!("- Connected to MQTT broker\n");

	// === 2. PLAYER SETUP ===
	let client_clone = client.clone();
	let alice_handler =
		async move { run_player(client_clone, "alice", "bob", false).await };
	let bob_handler = async move {
		// === 3. GAME INITIALIZATION ===
		// Give Alice time to subscribe first

		// DEMO SIMPLIFICATION: MQTT provides SUBACK confirmation for subscriptions,
		// but rumqttc doesn't expose this ACK in its API. Using sleep as workaround.
		// Production code should implement discovery patterns or use MQTT libraries
		// that provide subscription confirmation callbacks.
		tokio::time::sleep(Duration::from_millis(1000)).await;
		run_player(client, "bob", "alice", true).await
	};

	// === 4. CONCURRENT GAMEPLAY ===
	let _ = tokio::join!(alice_handler, bob_handler);

	// === 5. CLEANUP ===
	connection.shutdown().await?;
	println!("\n- Goodbye!");

	Ok(())
}
