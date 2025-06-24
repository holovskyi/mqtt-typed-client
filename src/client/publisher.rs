use std::marker::PhantomData;

use arcstr::ArcStr;
use rumqttc::{AsyncClient, QoS};

use super::error::MqttClientError;
use crate::message_serializer::MessageSerializer;

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
	// Note: For parametric topics, use #[mqtt_topic] macro which generates 
	// get_publisher() method with typed parameters
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
			.publish(self.topic.as_str(), self.qos, self.retain, payload)
			.await
			.map_err(MqttClientError::from)
	}
}
