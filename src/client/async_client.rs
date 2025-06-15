use std::time::Duration;

use arcstr::ArcStr;
use bytes::Bytes;
use rumqttc::Packet::{Disconnect, Publish};
use rumqttc::{AsyncClient, EventLoop, MqttOptions};
use rumqttc::{Event::Incoming, Event::Outgoing};
use tokio::time;
use tracing::{debug, error, info, warn};

use super::error::MqttClientError;
use super::publisher::TopicPublisher;
use super::subscriber::TypedSubscriber;
use crate::message_serializer::MessageSerializer;
use crate::routing::subscription_manager::SubscriptionConfig;
use crate::routing::{
	SubscriptionManagerActor, SubscriptionManagerController,
	SubscriptionManagerHandler,
};
use crate::topic::{self, TopicPatternPath, TopicRouterError};

#[derive(Clone)]
pub struct MqttClient<F> {
	pub client: AsyncClient,
	subscription_manager_handler: SubscriptionManagerHandler<Bytes>,
	serializer: F,
}

pub struct MqttConnection {
	pub client: AsyncClient,
	subscription_manager_controller: Option<SubscriptionManagerController>,
	event_loop_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<F> MqttClient<F>
where F: Default + Clone + Send + Sync + 'static
{
	pub async fn new(
		url: &str,
	) -> Result<(Self, MqttConnection), MqttClientError> {
		let mut mqttoptions = MqttOptions::parse_url(url)?;
		mqttoptions.set_keep_alive(Duration::from_secs(10));
		mqttoptions.set_clean_session(false);
		//TODO move 10 to config
		let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

		let (controller, handler) =
			SubscriptionManagerActor::spawn(client.clone());

		// Spawn the event loop in a separate task to handle MQTT messages
		// The event loop will terminate when it receives a Disconnect packet
		let handler_clone = handler.clone();
		let event_loop_handle = tokio::spawn(async move {
			Self::run(event_loop, handler_clone).await;
		});
		let fresh_client = Self {
			client: client.clone(),
			subscription_manager_handler: handler.clone(),
			serializer: F::default(),
		};
		let connection = MqttConnection {
			client,
			subscription_manager_controller: Some(controller),
			event_loop_handle: Some(event_loop_handle),
		};
		Ok((fresh_client, connection))
	}

	/// Main event loop that processes MQTT messages and handles graceful shutdown
	/// The loop terminates naturally when receiving a Disconnect packet (Incoming or Outgoing)
	async fn run(
		mut event_loop: EventLoop,
		subscription_manager: SubscriptionManagerHandler<Bytes>,
	) {
		let mut error_count = 0;
		const MAX_CONSECUTIVE_ERRORS: u32 = 10;
		const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
		const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

		// Main processing loop - continues until Disconnect packet is received
		// No explicit shutdown signal needed - MQTT protocol handles graceful termination
		loop {
			match event_loop.poll().await {
				| Ok(Incoming(Publish(p))) => {
					// Reset error count on successful message
					error_count = 0;

					debug!(topic = %p.topic, payload_size = p.payload.len(), "Received MQTT message");

					//let topic = Topic::from(p.topic);
					if let Err(err) = subscription_manager
						.send_data(p.topic, p.payload)
						.await
					{
						error!(error = ?err, "Failed to send data to subscription manager");
					}
				}
				| Ok(Incoming(Disconnect)) => {
					info!("Received MQTT Disconnect packet from server");
					// Server initiated disconnect - terminate gracefully
					break;
				}
				| Ok(Outgoing(rumqttc::Outgoing::Disconnect)) => {
					info!("Sent MQTT Disconnect packet to server");
					// Client initiated disconnect (via shutdown()) - terminate gracefully
					break;
				}
				| Ok(notification) => {
					// Reset error count on successful notification
					error_count = 0;
					debug!(notification = ?notification, "Received MQTT notification");
				}
				| Err(err) => {
					error_count += 1;
					error!(error_count = error_count, error = %err, "MQTT event loop error");

					if error_count >= MAX_CONSECUTIVE_ERRORS {
						error!(
							error_count = error_count,
							max_errors = MAX_CONSECUTIVE_ERRORS,
							"Too many consecutive errors, terminating event \
							 loop"
						);
						break;
					}

					// Exponential backoff with jitter
					let delay = INITIAL_RETRY_DELAY
						* 2_u32.pow((error_count - 1).min(10));
					let delay = delay.min(MAX_RETRY_DELAY);

					warn!(delay = ?delay, error_count = error_count, "Retrying MQTT connection");
					time::sleep(delay).await;
				}
			}
		}
		info!("MQTT event loop terminated gracefully");
		// Event loop naturally terminated after receiving Disconnect packet
		// This ensures all MQTT messages were properly processed before shutdown
	}

	pub fn get_publisher<T>(
		&self,
		topic: impl Into<ArcStr>,
	) -> Result<TopicPublisher<T, F>, topic::TopicRouterError>
	where
		F: MessageSerializer<T>,
	{
		let topic = topic.into();
		//Add type illegal topic
		validate_mqtt_topic(topic.as_str())?;
		Ok(TopicPublisher::new(
			self.client.clone(),
			self.serializer.clone(),
			topic,
		))
	}

	pub async fn subscribe<T>(
		&self,
		topic: impl Into<ArcStr>,
	) -> Result<TypedSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
	{
		self.subscribe_with_config(topic, SubscriptionConfig::default())
			.await
	}

	pub async fn subscribe_with_config<T>(
		&self,
		topic: impl Into<ArcStr>,
		config: SubscriptionConfig,
	) -> Result<TypedSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
	{
		let topic_pattern =
			TopicPatternPath::new_from_string(topic).map_err(|e| {
				MqttClientError::TopicPattern(format!(
					"Invalid topic pattern: {:?}",
					e
				))
			})?;
		let subscriber = self
			.subscription_manager_handler
			.subscribe(topic_pattern, config)
			.await?;
		Ok(TypedSubscriber::new(subscriber, self.serializer.clone()))
	}
}

impl MqttConnection {
	/// Gracefully shutdown the MQTT client by:
	/// 1. Shutting down subscription manager (sends unsubscribe commands for all topics)
	/// 2. Sending MQTT Disconnect packet (triggers event loop termination)
	/// 3. Waiting for event loop to finish processing
	pub async fn shutdown(mut self) -> Result<(), MqttClientError> {
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

// implement Drop for MqttAsyncClient to ensure graceful shutdown
impl Drop for MqttConnection {
	fn drop(&mut self) {
		if self.subscription_manager_controller.is_some()
			|| self.event_loop_handle.is_some()
		{
			error!(
				"MqttAsyncClient dropped without calling shutdown(). Please \
				 call shutdown() and await its completion before dropping."
			);
		}
	}
}
fn validate_mqtt_topic(topic_str: &str) -> Result<(), TopicRouterError> {
	//let topic_str = topic.as_ref();
	if topic_str.is_empty() || topic_str.len() > 65535 {
		return Err(TopicRouterError::invalid_routing_topic(
			topic_str,
			"Topic is empty or too long",
		));
	}
	if topic_str.chars().any(|c| matches!(c, '\0' | '#' | '+')) {
		return Err(TopicRouterError::invalid_routing_topic(
			topic_str,
			"Topic contains illegal characters ('#', '+', or null byte)",
		));
	}
	Ok(())
}
