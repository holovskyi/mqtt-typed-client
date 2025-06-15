use std::marker::PhantomData;
use std::sync::Arc;

use bytes::Bytes;

use super::error::MqttClientError;
use crate::message_serializer::MessageSerializer;
use crate::routing::Subscriber;
use crate::topic::topic_match::TopicMatch;

pub type IncomingMessage<T,F> = (Arc<TopicMatch>, Result<T,<F as MessageSerializer<T>>::DeserializeError>);

pub struct TypedSubscriber<T, F> {
	subscriber: Subscriber<Bytes>,
	serializer: F,
	_phantom: PhantomData<T>,
}

impl<T, F> TypedSubscriber<T, F>
where
	T: Send + Sync + 'static,
	F: MessageSerializer<T>,
{
	pub fn new(subscriber: Subscriber<Bytes>, serializer: F) -> Self {
		Self {
			subscriber,
			serializer,
			_phantom: PhantomData,
		}
	}

	pub async fn receive(
		&mut self,
	) -> Option<IncomingMessage<T, F>> {
		if let Some((topic, bytes)) = self.subscriber.recv().await {
			let message = self.serializer.deserialize(&bytes);
			Some((topic, message))
		} else {
			None
		}
	}

	pub async fn cancel(self) -> Result<(), MqttClientError> {
		self.subscriber
			.unsubscribe()
			.await
			.map_err(MqttClientError::Channel)
	}
}
