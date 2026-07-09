use std::marker::PhantomData;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc::error::SendError;
use tracing::warn;

use crate::ReceiveEvent;
use crate::message_meta::MessageMeta;
use crate::message_serializer::MessageSerializer;
use crate::routing::Subscriber;
use crate::topic::SubscriptionId;
use crate::topic::topic_match::TopicMatch;

/// A successfully delivered message: its decoded payload plus the context that
/// arrived with it. `topic` and `meta` are shared (`Arc`) across every
/// subscriber of the same publish; `payload` is this subscriber's own decoded
/// value.
#[derive(Debug)]
pub struct IncomingMessage<T> {
	/// The concrete topic the message arrived on (wildcards resolved).
	pub topic: Arc<TopicMatch>,
	/// Per-message protocol metadata (QoS, retain, dup, …).
	pub meta: Arc<MessageMeta>,
	/// The decoded payload.
	pub payload: T,
}

/// A message arrived but its payload could not be deserialized. Carries the same
/// context as [`IncomingMessage`] so a handler can still see which topic (and
/// with what metadata) failed. The stream continues after this event.
#[derive(Debug)]
pub struct DecodeFailure<E> {
	/// The concrete topic the undecodable message arrived on.
	pub topic: Arc<TopicMatch>,
	/// Per-message protocol metadata of the undecodable message.
	pub meta: Arc<MessageMeta>,
	/// The deserialization error.
	pub error: E,
}

/// A stream event from a typed subscriber (the value yielded by
/// [`MqttSubscriber::receive`]). Named an *event*, not a message, because it may
/// also be a [`ReceiveEvent::DecodeFailed`] or [`ReceiveEvent::Lagged`] notice.
///
/// `ReceiveEvent::Message` carries an [`IncomingMessage`]; `DecodeFailed`
/// carries a [`DecodeFailure`] so the topic and metadata are available even when
/// the payload could not be deserialized.
pub type SubscriberEvent<T, F> = ReceiveEvent<
	IncomingMessage<T>,
	DecodeFailure<<F as MessageSerializer<T>>::DeserializeError>,
>;

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

	/// Receive the next stream event from the subscription.
	///
	/// Returns `None` when the subscription is closed or cancelled. Lag notices
	/// from the low-level subscriber are forwarded unchanged; decoded payloads
	/// become `Message`, deserialization failures become `DecodeFailed`.
	pub async fn receive(&mut self) -> Option<SubscriberEvent<T, F>> {
		loop {
			match self.subscriber.recv().await? {
				| ReceiveEvent::Message((topic, meta, bytes)) => {
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

					return Some(match self.serializer.deserialize(&bytes) {
						| Ok(value) => ReceiveEvent::Message(IncomingMessage {
							topic,
							meta,
							payload: value,
						}),
						| Err(err) => {
							warn!(
								topic = %topic.topic_path(),
								payload_size = bytes.len(),
								error = ?err,
								"Failed to deserialize MQTT message payload"
							);
							ReceiveEvent::DecodeFailed(DecodeFailure {
								topic,
								meta,
								error: err,
							})
						}
					});
				}
				// The low-level subscriber's error slot is `Infallible`.
				| ReceiveEvent::DecodeFailed(never) => match never {},
				| ReceiveEvent::Lagged { missed } => {
					return Some(ReceiveEvent::Lagged { missed });
				}
			}
		}
	}

	/// Cancels subscription and unsubscribes from MQTT broker.
	pub async fn cancel(self) -> Result<(), SendError<SubscriptionId>> {
		self.subscriber.unsubscribe().await
	}

	/// Number of messages dropped for this subscription because the consumer
	/// could not keep up. See [`Subscriber::dropped_messages`].
	pub fn dropped_messages(&self) -> u64 {
		self.subscriber.dropped_messages()
	}
}
