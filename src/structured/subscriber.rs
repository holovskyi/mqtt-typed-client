use std::{marker::PhantomData, sync::Arc};

use thiserror::Error;
use tracing::{debug, error, info};
use crate::{topic::topic_match::TopicMatch, MessageSerializer, TypedSubscriber};
// use {
// 	BincodeSerializer, MessageSerializer, MqttClient, TypedSubscriber,
// 	topic::topic_match::TopicMatch,
// };

// enum for error message during recieving and conversion of incoming messages
#[derive(Error, Debug)]
pub enum MessageConversionError<DE> {
	#[error("Failed to deserialize payload: {0}")]
	PayloadDeserializationError(DE),

	#[error("Missing required parameter '{param}' at position {position}")]
	TopicParameterMissing { param: String, position: usize },

	#[error("Failed to parse parameter '{param}': {source}")]
	TopicParameterParseError {
		param: String,
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},
}

pub trait FromMqttMessage<T, DE> {
	fn from_mqtt_message(
		topic: Arc<TopicMatch>,
		payload: T,
	) -> Result<Self, MessageConversionError<DE>>
	where
		Self: Sized;
}

pub struct MqttStructuredSubscriber<MessageType, PayloadType, SerializerType> {
	inner: TypedSubscriber<PayloadType, SerializerType>,
	_phantom: PhantomData<MessageType>,
}

impl<MessageType, PayloadType, SerializerType>
	MqttStructuredSubscriber<MessageType, PayloadType, SerializerType>
where
	MessageType: FromMqttMessage<PayloadType, SerializerType::DeserializeError>,
	PayloadType: Send + Sync + 'static,
	SerializerType:
		Default + Clone + Send + Sync + MessageSerializer<PayloadType>,
{
	pub fn new(inner: TypedSubscriber<PayloadType, SerializerType>) -> Self {
		Self {
			inner,
			_phantom: PhantomData,
		}
	}

	pub async fn receive(
		&mut self
	) -> Option<
		Result<
			MessageType,
			MessageConversionError<SerializerType::DeserializeError>,
		>,
	> {
		if let Some((topic_match, payload_result)) = self.inner.receive().await
		{
			let result = match payload_result {
				| Ok(payload) => {
					MessageType::from_mqtt_message(topic_match, payload)
				}
				| Err(err) => Err(
					MessageConversionError::PayloadDeserializationError(err),
				),
			};
			Some(result)
		} else {
			None
		}
	}
}

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
