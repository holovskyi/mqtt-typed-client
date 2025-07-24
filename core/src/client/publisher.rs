use std::marker::PhantomData;

use arcstr::ArcStr;
use rumqttc::{AsyncClient, QoS};

use super::error::MqttClientError;
use crate::message_serializer::MessageSerializer;

/// Typed MQTT publisher for a specific topic.
///
/// Created via `MqttClient::get_publisher()`. Supports QoS and retain configuration.
pub struct MqttPublisher<T, F> {
	client: AsyncClient,
	topic: ArcStr,
	qos: QoS,
	retain: bool,
	serializer: F,
	_phantom: PhantomData<T>,
}

impl<T, F> MqttPublisher<T, F>
where F: MessageSerializer<T>
{
	/// Internal constructor. Use MqttClient::get_publisher() instead.
	pub fn new(
		client: AsyncClient,
		serializer: F,
		topic: impl Into<ArcStr>,
	) -> Self {
		Self {
			client,
			topic: topic.into(),
			qos: QoS::AtLeastOnce,
			retain: false,
			serializer,
			_phantom: PhantomData,
		}
	}
	/// Sets Quality of Service level for published messages.
	pub fn with_qos(mut self, qos: QoS) -> Self {
		self.qos = qos;
		self
	}

	/// Sets retain flag for published messages.
	pub fn with_retain(mut self, retain: bool) -> Self {
		self.retain = retain;
		self
	}

	/// Get the topic this publisher is configured for.
	pub fn topic(&self) -> &ArcStr {
		&self.topic
	}

	/// Get qos level for this publisher.
	pub fn qos(&self) -> QoS {
		self.qos
	}

	/// Get retain flag for this publisher.
	pub fn retain(&self) -> bool {
		self.retain
	}

	/// Publishes data to the configured topic.
	pub async fn publish(&self, data: &T) -> Result<(), MqttClientError> {
		self.publish_with_retain_override(data, self.retain).await
	}

	/// Publishes data with retain flag explicitly set to true.
	pub async fn publish_retain(&self, data: &T) -> Result<(), MqttClientError> {
		self.publish_with_retain_override(data, true).await
	}

	/// Publishes data with retain flag explicitly set to false.
	pub async fn publish_normal(&self, data: &T) -> Result<(), MqttClientError> {
		self.publish_with_retain_override(data, false).await
	}

	/// Internal helper to avoid code duplication
	async fn publish_with_retain_override(&self, data: &T, retain: bool) -> Result<(), MqttClientError> {
		let payload = self
			.serializer
			.serialize(data)
			.map_err(|e| MqttClientError::Serialization(format!("{e:?}")))?;
		self.client
			.publish(self.topic.as_str(), self.qos, retain, payload)
			.await
			.map_err(MqttClientError::from)
	}

	/// Clear retained message for this topic
	///
	/// Sends an empty payload with retain=true to remove any retained message.
	/// Uses the same QoS level as configured for this publisher.
	pub async fn clear_retained(&self) -> Result<(), MqttClientError> {
		self.client
			.publish(self.topic.as_str(), self.qos, true, Vec::new())
			.await
			.map_err(MqttClientError::from)
	}
}
