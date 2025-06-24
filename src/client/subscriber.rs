use std::marker::PhantomData;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc::error::SendError;

use crate::message_serializer::MessageSerializer;
use crate::routing::Subscriber;
use crate::topic::topic_match::TopicMatch;
use crate::topic::SubscriptionId;

/// Message received from MQTT topic with deserialization result.
pub type IncomingMessage<T, F> = (
	Arc<TopicMatch>,
	Result<T, <F as MessageSerializer<T>>::DeserializeError>,
);

/// Typed MQTT subscriber for topic patterns.
///
/// Created via `MqttClient::subscribe()`. Automatically deserializes messages.
pub struct MqttSubscriber<T, F> {
	subscriber: Subscriber<Bytes>,
	serializer: F,
	_phantom: PhantomData<T>,
}

impl<T, F> MqttSubscriber<T, F>
where
	T: Send + Sync + 'static,
	F: MessageSerializer<T>,
{
	/// Creates typed subscriber from raw byte subscriber.
	pub fn new(subscriber: Subscriber<Bytes>, serializer: F) -> Self {
		Self {
			subscriber,
			serializer,
			_phantom: PhantomData,
		}
	}

	/// Receive next message from subscription.
	///
	/// Returns `None` when subscription is closed or cancelled.
	pub async fn receive(&mut self) -> Option<IncomingMessage<T, F>> {
		if let Some((topic, bytes)) = self.subscriber.recv().await {
			let message = self.serializer.deserialize(&bytes);
			Some((topic, message))
		} else {
			None
		}
	}

	/// Cancels subscription and unsubscribes from MQTT broker.
	pub async fn cancel(self) -> Result<(), SendError<SubscriptionId>> {
		self.subscriber.unsubscribe().await
	}
}
