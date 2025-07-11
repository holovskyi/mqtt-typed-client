//! MQTT connection management module
//!
//! This module provides the connection lifecycle management separated from
//! the main client interface for cleaner API separation.

use rumqttc::AsyncClient;
use tracing::{error, warn};

use crate::routing::SubscriptionManagerController;

/// MQTT connection handle for lifecycle management
///
/// This type manages the connection lifecycle and provides graceful shutdown.
/// It should be kept alive for the duration of the MQTT session.
pub struct MqttConnection {
	client: AsyncClient,
	subscription_manager_controller: Option<SubscriptionManagerController>,
	event_loop_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MqttConnection {
	/// Create a new connection handle
	pub(crate) fn new(
		client: AsyncClient,
		subscription_manager_controller: SubscriptionManagerController,
		event_loop_handle: tokio::task::JoinHandle<()>,
	) -> Self {
		Self {
			client,
			subscription_manager_controller: Some(
				subscription_manager_controller,
			),
			event_loop_handle: Some(event_loop_handle),
		}
	}

	/// Gracefully shutdown the MQTT connection by:
	/// 1. Shutting down subscription manager (sends unsubscribe commands for all topics)
	/// 2. Sending MQTT Disconnect packet (triggers event loop termination)
	/// 3. Waiting for event loop to finish processing
	pub async fn shutdown(mut self) -> Result<(), crate::MqttClientError> {
		// Step 1: Shutdown subscription manager first to clean up subscriptions
		// This will send unsubscribe commands to the broker for all active topics
		if let Some(controller) = self.subscription_manager_controller.take() {
			// Ensure we have a controller to shutdown
			if let Err(e) = controller.shutdown().await {
				warn!(error = %e, "Failed to shutdown subscription manager");
			}
		} else {
			warn!("No subscription manager controller available for shutdown");
		}

		// Step 2: Send Disconnect packet to MQTT broker
		// This will cause the event loop to receive Outgoing(Disconnect) and break
		if let Err(e) = self.client.disconnect().await {
			warn!(error = %e, "Failed to disconnect MQTT client");
		}

		// Step 3: Wait for event loop to terminate naturally after processing Disconnect
		if let Some(handle) = self.event_loop_handle.take() {
			// Ensure we have a handle to wait on
			if let Err(e) = handle.await {
				warn!(error = %e, "Event loop task failed");
			}
		} else {
			warn!("No event loop handle available to await");
		}

		Ok(())
	}
}

// implement Drop for MqttConnection to ensure graceful shutdown
impl Drop for MqttConnection {
	fn drop(&mut self) {
		if self.subscription_manager_controller.is_some()
			|| self.event_loop_handle.is_some()
		{
			error!(
				"MqttConnection dropped without calling shutdown(). Please \
				 call shutdown() and await its completion before dropping."
			);
		}
	}
}
