use std::marker::PhantomData;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc::error::SendError;
use tracing::warn;

use crate::message_serializer::MessageSerializer;
use crate::routing::Subscriber;
use crate::topic::SubscriptionId;
use crate::topic::topic_match::TopicMatch;

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
		loop {
			if let Some((topic, bytes)) = self.subscriber.recv().await {
				// TODO: Flexible mechanism for handling empty payloads (retain clear events)
				// 
				// Proposed approach:
				// - Regular types (payload: MyMessage) ignore empty payloads (95% of cases)
				// - Optional types (payload: Option<MyMessage>) receive None for clear events (5% of cases)
				//
				// Implementation requires:
				// 1. Add MessageSerializer<Option<T>> impl for all serializers 
				// 2. Empty bytes deserialize to Ok(None)
				// 3. None serializes to empty Vec<u8>
				//
				// Example usage:
				// #[mqtt_topic("device/{id}")]
				// struct RegularTopic { id: String, payload: Status }           // ignores clears
				// 
				// #[mqtt_topic("device/{id}")] 
				// struct ClearAwareTopic { id: String, payload: Option<Status> } // receives None on clear
				//
				// For now: ignore empty payloads and log at debug level
				if bytes.is_empty() {
					tracing::debug!(
						topic = %topic.topic_path(),
						"Ignoring empty MQTT payload (likely retain clear event)"
					);
					continue; // Skip empty payloads and wait for next message
				}

				let message = self.serializer.deserialize(&bytes);

				// Log deserialization attempts and failures
				match &message {
					| Ok(_) => (),
					| Err(err) => {
						warn!(
							topic = %topic.topic_path(),
							payload_size = bytes.len(),
							error = ?err,
							"Failed to deserialize MQTT message payload"
						);
					}
				}

				return Some((topic, message));
			} else {
				return None;
			}
		}
	}

	/// Cancels subscription and unsubscribes from MQTT broker.
	pub async fn cancel(self) -> Result<(), SendError<SubscriptionId>> {
		self.subscriber.unsubscribe().await
	}
}
