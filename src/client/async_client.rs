use std::time::Duration;

use bytes::Bytes;
use rumqttc::Packet::{Disconnect, Publish};
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use rumqttc::{Event::Incoming, Event::Outgoing};
use string_cache::DefaultAtom as Topic;
use tokio::time;

use super::error::MqttClientError;
use super::publisher::TopicPublisher;
use super::subscriber::TypedSubscriber;
use crate::message_serializer::MessageSerializer;
use crate::routing::{
	SubscriptionManagerActor, SubscriptionManagerController,
	SubscriptionManagerHandler,
};
use crate::topic::{self, TopicPatternPath, TopicRouterError};

pub struct MqttAsyncClient<F> {
	pub client: AsyncClient,
	subscription_manager_handler: SubscriptionManagerHandler<Bytes>,
	subscription_manager_controller: Option<SubscriptionManagerController>,
	event_loop_handle: Option<tokio::task::JoinHandle<()>>,
	serializer: F,
}

impl<F> MqttAsyncClient<F>
where F: Default + Clone + Send + Sync + 'static
{
	pub async fn new(url: &str) -> Result<Self, MqttClientError> {
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
			client,
			subscription_manager_handler: handler.clone(),
			subscription_manager_controller: Some(controller),
			event_loop_handle: Some(event_loop_handle),
			serializer: F::default(),
		};
		Ok(fresh_client)
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

					println!("INCOMING: {}", p.topic);

					let topic = Topic::from(p.topic);
					if let Err(err) =
						subscription_manager.send_data(topic, p.payload).await
					{
						eprintln!(
							"Failed to send data to subscription manager: \
							 {err:?}"
						)
					}
				}
				| Ok(Incoming(Disconnect)) => {
					println!("INCOMING Disconnect пакет");
					// Server initiated disconnect - terminate gracefully
					break;
				}
				| Ok(Outgoing(rumqttc::Outgoing::Disconnect)) => {
					println!("OUTGOING Disconnect пакет");
					// Client initiated disconnect (via shutdown()) - terminate gracefully
					break;
				}
				| Ok(notification) => {
					// Reset error count on successful notification
					error_count = 0;
					eprintln!("Received = {notification:?}");
				}
				| Err(err) => {
					error_count += 1;
					eprintln!("MQTT event loop error #{error_count}: {err:?}");

					if error_count >= MAX_CONSECUTIVE_ERRORS {
						eprintln!(
							"Too many consecutive errors ({error_count}), \
							 terminating event loop"
						);
						break;
					}

					// Exponential backoff with jitter
					let delay = INITIAL_RETRY_DELAY
						* 2_u32.pow((error_count - 1).min(10));
					let delay = delay.min(MAX_RETRY_DELAY);

					eprintln!("Retrying in {delay:?}...");
					time::sleep(delay).await;
				}
			}
		}
		eprintln!("MQTT event loop terminated");
		// Event loop naturally terminated after receiving Disconnect packet
		// This ensures all MQTT messages were properly processed before shutdown
	}

	pub fn get_publisher<T>(
		&self,
		topic: &str,
	) -> Result<TopicPublisher<T, F>, topic::TopicRouterError>
	where
		F: MessageSerializer<T>,
	{
		//Add type illegal topic
		validate_mqtt_topic(topic)?;
		Ok(TopicPublisher::new(
			self.client.clone(),
			self.serializer.clone(),
			topic,
		))
	}

	pub async fn subscribe<T>(
		&self,
		topic: &str,
	) -> Result<TypedSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
	{
		let topic_pattern = TopicPatternPath::try_from(topic).map_err(|e| {
			MqttClientError::TopicPattern(format!(
				"Invalid topic pattern '{}': {:?}",
				topic, e
			))
		})?;
		let subscriber = self
			.subscription_manager_handler
			.subscribe(topic_pattern)
			.await?;
		Ok(TypedSubscriber::new(subscriber, self.serializer.clone()))
	}

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
				eprintln!(
					"Warning: Failed to shutdown subscription manager: {}",
					e
				);
			}
		} else {
			eprintln!(
				"Warning: No subscription manager controller available for \
				 shutdown"
			);
		}

		// Step 2: Send Disconnect packet to MQTT broker
		// This will cause the event loop to receive Outgoing(Disconnect) and break
		if let Err(e) = self.client.disconnect().await {
			eprintln!("Warning: Failed to disconnect MQTT client: {}", e);
		}

		// Step 3: Wait for event loop to terminate naturally after processing Disconnect
		if let Some(handle) = self.event_loop_handle.take() {
			// Ensure we have a handle to wait on
			if let Err(e) = handle.await {
				eprintln!("Warning: Event loop task failed: {}", e);
			}
		} else {
			eprintln!("Warning: No event loop handle available to await");
		}

		Ok(())
	}
}

// implement Drop for MqttAsyncClient to ensure graceful shutdown
impl<F> Drop for MqttAsyncClient<F> {
	fn drop(&mut self) {
		if self.subscription_manager_controller.is_some()
			|| self.event_loop_handle.is_some()
		{
			eprintln!(
				"Warning: MqttAsyncClient dropped without calling shutdown(). \
				 Please call shutdown() and await its completion before \
				 dropping."
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
