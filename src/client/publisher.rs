use std::marker::PhantomData;

use rumqttc::{AsyncClient, QoS};

use super::error::MqttClientError;
use crate::message_serializer::MessageSerializer;

pub struct TopicPublisher<T, F> {
	client: AsyncClient,
	topic: String,
	qos: QoS,
	retain: bool,
	serializer: F,
	_phantom: PhantomData<T>,
}

impl<T, F> TopicPublisher<T, F>
where F: MessageSerializer<T>
{
	pub fn new(client: AsyncClient, serializer: F, topic: &str) -> Self {
		Self {
			client,
			topic: topic.to_string(),
			qos: QoS::AtLeastOnce,
			retain: false,
			serializer,
			_phantom: PhantomData,
		}
	}
	pub fn with_qos(mut self, qos: QoS) -> Self {
		self.qos = qos;
		self
	}

	pub fn with_retain(mut self, retain: bool) -> Self {
		self.retain = retain;
		self
	}

	pub async fn publish(&self, data: &T) -> Result<(), MqttClientError> {
		let payload = self
			.serializer
			.serialize(data)
			.map_err(|e| MqttClientError::Serialization(format!("{:?}", e)))?;
		self.client
			.publish(&self.topic, self.qos, self.retain, payload)
			.await
			.map_err(MqttClientError::from)
	}
}
