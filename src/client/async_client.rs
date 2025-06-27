use std::time::Duration;

use arcstr::ArcStr;
use bytes::Bytes;
use rumqttc::Packet::{Disconnect, Publish};
use rumqttc::{AsyncClient, EventLoop, MqttOptions};
use rumqttc::{Event::Incoming, Event::Outgoing};
use tokio::time;
use tracing::{debug, error, info, warn};

use super::config::MqttClientConfig;
use super::error::MqttClientError;
use super::publisher::MqttPublisher;
use super::subscriber::MqttSubscriber;
use crate::connection::MqttConnection;
use crate::message_serializer::MessageSerializer;
use crate::routing::subscription_manager::SubscriptionConfig;
use crate::routing::{SubscriptionManagerActor, SubscriptionManagerHandler};
use crate::topic::{TopicPatternPath, TopicError};

/// Type-safe MQTT client with automatic subscription management.
///
/// Provides typed publishers and subscribers with automatic serialization.
/// Connection lifecycle is managed separately via `MqttConnection`.
#[derive(Clone)]
pub struct MqttClient<F> {
	client: AsyncClient,
	subscription_manager_handler: SubscriptionManagerHandler<Bytes>,
	serializer: F,
}

impl<F> MqttClient<F>
where F: Default + Clone + Send + Sync + 'static
{
	/// Create MQTT client with default configuration.
	///
	/// Returns both client and connection handle. Keep connection alive
	/// for the session duration, call `connection.shutdown()` when done.
	pub async fn connect(
		url: &str,
	) -> Result<(Self, MqttConnection), MqttClientError> {
		Self::connect_with_config(url, MqttClientConfig::default()).await
	}

	/// Create a new MQTT client with custom configuration
	pub async fn connect_with_config(
		url: &str,
		config: MqttClientConfig,
	) -> Result<(Self, MqttConnection), MqttClientError> {
		let topic_path_cache_capacity = std::num::NonZeroUsize::new(
			config.topic_cache_size,
		)
		.ok_or_else(|| {
			MqttClientError::ConfigurationValue(
				"topic_cache_size must be greater than 0".to_string(),
			)
		})?;
		let mut mqttoptions = MqttOptions::parse_url(url)?;
		mqttoptions.set_keep_alive(Duration::from_secs(10));
		mqttoptions.set_clean_session(false);
		let (client, event_loop) =
			AsyncClient::new(mqttoptions, config.event_loop_capacity);

		let (controller, handler) = SubscriptionManagerActor::spawn(
			client.clone(),
			topic_path_cache_capacity,
			config.command_channel_capacity,
			config.unsubscribe_channel_capacity,
		);

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
		let connection =
			MqttConnection::new(client, controller, event_loop_handle);
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
					if let Err(err) =
						subscription_manager.send_data(p.topic, p.payload).await
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

	/// Create typed publisher for specific topic.
	///
	/// Topic must not contain wildcard characters (`+`, `#`).
	pub fn get_publisher<T>(
		&self,
		topic: impl Into<ArcStr>,
	) -> Result<MqttPublisher<T, F>, TopicError>
	where
		F: MessageSerializer<T>,
	{
		let topic = topic.into();
		//Add type illegal topic
		validate_mqtt_topic(topic.as_str())?;
		Ok(MqttPublisher::new(
			self.client.clone(),
			self.serializer.clone(),
			topic,
		))
	}

	/// Subscribe to topic pattern with default configuration.
	///
	/// Supports MQTT wildcards: `+` (single level), `#` (multi-level).
	pub async fn subscribe<T>(
		&self,
		topic: impl Into<ArcStr>,
	) -> Result<MqttSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
	{
		self.subscribe_with_config(topic, SubscriptionConfig::default())
			.await
	}

	/// Subscribe with custom configuration (QoS, caching strategy)
	pub async fn subscribe_with_config<T>(
		&self,
		topic: impl Into<ArcStr>,
		config: SubscriptionConfig,
	) -> Result<MqttSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
	{
		let topic_pattern =
			TopicPatternPath::new_from_string(topic, config.cache_strategy)?;
		let subscriber = self
			.subscription_manager_handler
			.subscribe(topic_pattern, config)
			.await?;
		Ok(MqttSubscriber::new(subscriber, self.serializer.clone()))
	}
}

fn validate_mqtt_topic(topic_str: &str) -> Result<(), TopicError> {
	//let topic_str = topic.as_ref();
	if topic_str.is_empty() || topic_str.len() > 65535 {
		return Err(crate::topic::TopicRouterError::invalid_routing_topic(
			topic_str,
			"Topic is empty or too long",
		).into());
	}
	if topic_str.chars().any(|c| matches!(c, '\0' | '#' | '+')) {
		return Err(crate::topic::TopicRouterError::invalid_routing_topic(
			topic_str,
			"Topic contains illegal characters ('#', '+', or null byte)",
		).into());
	}
	Ok(())
}
