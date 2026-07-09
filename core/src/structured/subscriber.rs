//! Structured MQTT subscribers with automatic topic parameter extraction.

use std::{marker::PhantomData, sync::Arc};

use thiserror::Error;

use crate::{
	DecodeFailure, IncomingMessage, MessageMeta, MessageSerializer,
	MqttSubscriber, ReceiveEvent, topic::topic_match::TopicMatch,
};
// use {
// 	BincodeSerializer, MessageSerializer, MqttClient, TypedSubscriber,
// 	topic::topic_match::TopicMatch,
// };

// enum for error message during recieving and conversion of incoming messages
/// Errors that occur during message conversion from MQTT topics.
#[derive(Error, Debug)]
pub enum MessageConversionError<DE> {
	/// Failed to deserialize message payload
	#[error("Failed to deserialize payload: {0}")]
	PayloadDeserializationError(DE),

	/// Topic parameter expected but not found
	#[error("Missing required parameter '{param}' at position {position}")]
	TopicParameterMissing {
		/// Parameter name
		param: String,
		/// Wildcard position in pattern
		position: usize,
	},

	/// Topic parameter found but couldn't parse to target type
	#[error("Failed to parse parameter '{param}': {source}")]
	TopicParameterParseError {
		/// Parameter name
		param: String,
		/// Parse error details
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},
}

/// Trait for converting MQTT messages into structured types.
///
/// Typically implemented via `#[mqtt_topic]` macro for automatic topic parameter extraction.
pub trait FromMqttMessage<T, DE> {
	/// Convert an MQTT topic, its metadata, and payload into a structured message
	fn from_mqtt_message(
		topic: Arc<TopicMatch>,
		meta: Arc<MessageMeta>,
		payload: T,
	) -> Result<Self, MessageConversionError<DE>>
	where
		Self: Sized;
}

/// Structured MQTT subscriber with automatic topic parameter extraction.
///
/// Created via `#[mqtt_topic]` macro. Converts raw MQTT messages into structured types.
pub struct MqttTopicSubscriber<MessageType, PayloadType, SerializerType> {
	inner: MqttSubscriber<PayloadType, SerializerType>,
	_phantom: PhantomData<MessageType>,
}

impl<MessageType, PayloadType, SerializerType>
	MqttTopicSubscriber<MessageType, PayloadType, SerializerType>
where
	MessageType: FromMqttMessage<PayloadType, SerializerType::DeserializeError>,
	PayloadType: Send + Sync + 'static,
	SerializerType:
		Default + Clone + Send + Sync + MessageSerializer<PayloadType>,
{
	/// Creates a structured subscriber from a typed MQTT subscriber.
	pub fn new(inner: MqttSubscriber<PayloadType, SerializerType>) -> Self {
		Self {
			inner,
			_phantom: PhantomData,
		}
	}

	/// Receives and converts the next MQTT stream event into the structured type.
	///
	/// Returns `None` when the subscription is closed. `Message` is a fully
	/// decoded value; a deserialization or topic-parameter failure becomes
	/// `DecodeFailed`; dropped-message notices are forwarded as `Lagged`.
	pub async fn receive(
		&mut self,
	) -> Option<
		ReceiveEvent<
			MessageType,
			MessageConversionError<SerializerType::DeserializeError>,
		>,
	> {
		match self.inner.receive().await? {
			| ReceiveEvent::Message(IncomingMessage {
				topic,
				meta,
				payload,
			}) => Some(
				match MessageType::from_mqtt_message(topic, meta, payload) {
					| Ok(message) => ReceiveEvent::Message(message),
					| Err(err) => ReceiveEvent::DecodeFailed(err),
				},
			),
			| ReceiveEvent::DecodeFailed(DecodeFailure { error, .. }) => {
				Some(ReceiveEvent::DecodeFailed(
					MessageConversionError::PayloadDeserializationError(error),
				))
			}
			| ReceiveEvent::Lagged { missed } => {
				Some(ReceiveEvent::Lagged { missed })
			}
		}
	}

	/// Number of messages dropped for this subscription because the consumer
	/// could not keep up. See [`SubscriptionConfig`](crate::SubscriptionConfig)
	/// for the backpressure policy that governs drops.
	pub fn dropped_messages(&self) -> u64 {
		self.inner.dropped_messages()
	}
}

/// Extract and parse a topic parameter by wildcard index
pub fn extract_topic_parameter<T, DE>(
	topic: &TopicMatch,
	index: usize,
	param_name: &str,
) -> Result<T, MessageConversionError<DE>>
where
	T: std::str::FromStr,
	T::Err: std::error::Error + Send + Sync + 'static,
{
	topic
		.get_param(index)
		.ok_or_else(|| MessageConversionError::TopicParameterMissing {
			param: param_name.to_string(),
			position: index,
		})
		.and_then(|param| {
			param.parse::<T>().map_err(|e| {
				MessageConversionError::TopicParameterParseError {
					param: param_name.to_string(),
					source: Box::new(e),
				}
			})
		})
}
