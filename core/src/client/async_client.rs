use std::time::Duration;

use arcstr::ArcStr;
use bytes::Bytes;
use rumqttc::Packet::{self, Disconnect, Publish};
use rumqttc::{AsyncClient, ConnAck, ConnectReturnCode, EventLoop};
use rumqttc::{Event::Incoming, Event::Outgoing};
use tokio::time;
use tracing::{debug, error, info, warn};

use super::config::MqttClientConfig;
use super::error::MqttClientError;
use super::publisher::MqttPublisher;
use super::subscriber::MqttSubscriber;
use crate::client::error::ConnectionEstablishmentError;
use crate::connection::MqttConnection;
use crate::message_serializer::MessageSerializer;
use crate::routing::subscription_manager::SubscriptionConfig;
use crate::routing::{SubscriptionManagerActor, SubscriptionManagerHandler};
use crate::topic::{TopicError, TopicPatternPath};

/// Type-safe MQTT client with automatic subscription management.
///
/// Provides typed publishers and subscribers with automatic serialization.
/// Connection lifecycle is managed separately via `MqttConnection`.
#[derive(Clone, Debug)]
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
		let config = MqttClientConfig::<F>::from_url(url)?;
		Self::connect_with_config(config).await
	}

	/// Create a new MQTT client with custom configuration
	pub async fn connect_with_config(
		config: MqttClientConfig<F>,
	) -> Result<(Self, MqttConnection), MqttClientError> {
		let topic_path_cache_capacity =
			std::num::NonZeroUsize::new(config.settings.topic_cache_size)
				.ok_or_else(|| {
					MqttClientError::ConfigurationValue(
						"topic_cache_size must be greater than 0".to_string(),
					)
				})?;
		// Use the provided MqttOptions directly - no more hardcoded values!
		let (client, new_event_loop) = AsyncClient::new(
			config.connection,
			config.settings.event_loop_capacity,
		);

		let timeout_millis = config.settings.connection_timeout_millis;
		let connection_timeout = Duration::from_millis(timeout_millis);
		let connected_event_loop = tokio::time::timeout(
			connection_timeout,
			Self::establish_connection(new_event_loop),
		)
		.await
		.map_err(|_| ConnectionEstablishmentError::Timeout { timeout_millis })?
		.map_err(MqttClientError::ConnectionEstablishment)?;

		let (controller, handler) = SubscriptionManagerActor::spawn(
			client.clone(),
			topic_path_cache_capacity,
			config.settings.command_channel_capacity,
			config.settings.unsubscribe_channel_capacity,
		);

		// Spawn the event loop in a separate task to handle MQTT messages
		// The event loop will terminate when it receives a Disconnect packet
		let handler_clone = handler.clone();
		let event_loop_handle = tokio::spawn(async move {
			Self::run(connected_event_loop, handler_clone).await;
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

	async fn establish_connection(
		mut event_loop: EventLoop,
	) -> Result<EventLoop, ConnectionEstablishmentError> {
		loop {
			match event_loop.poll().await {
				| Ok(Incoming(Packet::ConnAck(ConnAck { code, .. }))) => {
					if code == ConnectReturnCode::Success {
						debug!("MQTT connection established successfully");
						return Ok(event_loop);
					} else {
						debug!(code = ?code, "MQTT connection rejected by broker");
						return Err(
							ConnectionEstablishmentError::BrokerRejected {
								code,
							},
						);
					}
				}
				| Ok(notification) => {
					debug!(notification = ?notification, "Bootstrap phase notification");
				}
				| Err(connection_err) => {
					debug!(error = %connection_err, "MQTT connection error during bootstrap phase");
					return Err(ConnectionEstablishmentError::Network(
						connection_err,
					));
				}
			}
		}
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
				| Ok(Incoming(Packet::ConnAck(ConnAck {
					session_present: false,
					code: ConnectReturnCode::Success,
				}))) => {
					info!(
						"MQTT reconnected without session, resubscribing to \
						 all topics"
					);
					let _ = subscription_manager
						.resubscribe_all()
						.await
						.inspect_err(|err| {
							error!(error = ?err, "Failed to resubscribe to topics");
						});
				}
				| Ok(Incoming(Packet::ConnAck(ConnAck {
					session_present: true,
					code: ConnectReturnCode::Success,
				}))) => {
					info!(
						"MQTT reconnected with session preserved, \
						 subscriptions maintained by broker"
					);
				}
				| Ok(Incoming(Publish(p))) => {
					// Reset error count on successful message
					error_count = 0;

					debug!(topic = %p.topic, payload_size = p.payload.len(), "Received MQTT message");

					//let topic = Topic::from(p.topic);
					if let Err(err) = subscription_manager
						.dispatch_incoming_message(p.topic, p.payload)
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
					debug!(notification = ?notification, "Received OTHER MQTT notification");
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
		topic: impl TryInto<TopicPatternPath, Error: Into<MqttClientError>>,
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
		topic: impl TryInto<TopicPatternPath, Error: Into<MqttClientError>>,
		config: SubscriptionConfig,
	) -> Result<MqttSubscriber<T, F>, MqttClientError>
	where
		T: 'static + Send + Sync,
		F: MessageSerializer<T>,
		//TP: TryInto<TopicPatternPath>,
		//TP::Error: Into<MqttClientError>
	{
		let topic_pattern = topic.try_into().map_err(Into::into)?;
		//TopicPatternPath::new_from_string(topic, config.cache_strategy)?;
		let subscriber = self
			.subscription_manager_handler
			.create_subscription(topic_pattern, config)
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
		)
		.into());
	}
	if topic_str.chars().any(|c| matches!(c, '\0' | '#' | '+')) {
		return Err(crate::topic::TopicRouterError::invalid_routing_topic(
			topic_str,
			"Topic contains illegal characters ('#', '+', or null byte)",
		)
		.into());
	}
	Ok(())
}
