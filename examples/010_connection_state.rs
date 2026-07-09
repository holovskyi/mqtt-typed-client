//! # Connection State Observability - MQTT Typed Client
//!
//! Demonstrates [`MqttClient::connection_state`]: a `tokio::sync::watch`
//! channel that reports the connection lifecycle
//! (`Connected` / `Reconnecting` / `Disconnected`).
//!
//! A background task watches the channel and logs every transition. The channel
//! is level-triggered (latest state wins; rapid transitions may collapse), but
//! the terminal `Disconnected` is never missed. Here we drive a clean
//! `shutdown()` and watch the observer see `Disconnected { CleanShutdown }`.

mod shared;

use std::time::Duration;

use mqtt_typed_client::{
	BincodeSerializer, ConnectionState, DisconnectReason, MqttClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	shared::tracing::setup(None);

	let connection_url = shared::config::build_url("connection_state");
	println!("Connecting to MQTT broker: {connection_url}");

	let (client, connection) =
		MqttClient::<BincodeSerializer>::connect(&connection_url)
			.await
			.inspect_err(|e| {
				shared::config::print_connection_error(&connection_url, e);
			})?;

	// Watch the connection lifecycle on a background task.
	let mut state_rx = client.connection_state();
	println!("Initial state: {:?}", *state_rx.borrow());

	let watcher = tokio::spawn(async move {
		// `changed()` errors only when the sender is gone; the loop also exits
		// on the terminal `Disconnected` (after which `changed()` never fires).
		while state_rx.changed().await.is_ok() {
			let state = state_rx.borrow().clone();
			println!("Connection state -> {state:?}");
			// `ConnectionState`/`DisconnectReason` are `#[non_exhaustive]`, so
			// external matches need a trailing `..` / wildcard — future v5
			// fields and variants stay non-breaking.
			if let ConnectionState::Disconnected { reason, .. } = &state {
				match reason {
					| DisconnectReason::CleanShutdown => {
						println!("   (clean shutdown requested by us)")
					}
					| DisconnectReason::BrokerDisconnected { .. } => {
						println!("   (broker sent DISCONNECT)")
					}
					| DisconnectReason::MaxErrorsExceeded {
						errors, ..
					} => {
						println!("   ({errors} consecutive errors)")
					}
					| _ => println!("   (other)"),
				}
				break;
			}
		}
	});

	// Do some normal work so the connection is genuinely live for a moment.
	tokio::time::sleep(Duration::from_millis(200)).await;

	// Graceful shutdown drives the terminal transition.
	connection.shutdown().await?;

	// The watcher observes `Disconnected { CleanShutdown }` and exits.
	let _ = watcher.await;
	println!("\nGoodbye!");
	Ok(())
}
