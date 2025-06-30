use rumqttc::{ClientError, OptionError};
use tokio::sync::mpsc::error::SendError;

use crate::{
	routing::SubscriptionError, 
	topic::{topic_pattern_path::TopicFormatError, SubscriptionId, TopicError, TopicPatternError}
};

/// Errors that can occur in MQTT client operations
#[derive(Debug)]
pub enum MqttClientError {
	/// Connection-related errors from rumqttc
	Connection(ClientError),
	/// Configuration errors when parsing MQTT options
	Configuration(OptionError),
	/// Invalid configuration parameter values
	ConfigurationValue(String),
	/// Serialization errors when converting data to bytes
	Serialization(String),
	/// Subscription management errors
	Subscription(SubscriptionError),
	/// Topic-related errors (pattern, matching, routing)
	Topic(TopicError),
	/// Channel communication errors
	UnsubscribeFailed(SubscriptionId),
}

impl MqttClientError {
	/// Create a TopicPattern error
	pub fn topic_pattern(err: TopicPatternError) -> Self {
		MqttClientError::Topic(TopicError::Pattern(err))
	}
}

impl std::fmt::Display for MqttClientError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			| MqttClientError::Connection(e) => {
				write!(f, "Connection error: {e}")
			}
			| MqttClientError::Configuration(e) => {
				write!(f, "Configuration error: {e}")
			}
			| MqttClientError::ConfigurationValue(e) => {
				write!(f, "Invalid configuration value: {e}")
			}
			| MqttClientError::Serialization(e) => {
				write!(f, "Serialization error: {e}")
			}
			| MqttClientError::Subscription(e) => {
				write!(f, "Subscription error: {e:?}")
			}
			| MqttClientError::Topic(e) => {
				write!(f, "Topic error: {e}")
			}
			| MqttClientError::UnsubscribeFailed(s_id) => write!(
				f,
				"Failed to unsubscribe: subscription {s_id} channel closed"
			),
		}
	}
}

impl std::error::Error for MqttClientError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			| MqttClientError::Connection(e) => Some(e),
			| MqttClientError::Configuration(e) => Some(e),
			| _ => None,
		}
	}
}

impl From<ClientError> for MqttClientError {
	fn from(err: ClientError) -> Self {
		MqttClientError::Connection(err)
	}
}

impl From<OptionError> for MqttClientError {
	fn from(err: OptionError) -> Self {
		MqttClientError::Configuration(err)
	}
}

impl From<SubscriptionError> for MqttClientError {
	fn from(err: SubscriptionError) -> Self {
		MqttClientError::Subscription(err)
	}
}

impl From<TopicError> for MqttClientError {
	fn from(err: TopicError) -> Self {
		MqttClientError::Topic(err)
	}
}

impl From<TopicFormatError> for MqttClientError {
    fn from(err: TopicFormatError) -> Self {
        MqttClientError::Topic(TopicError::Format(err))  
    }
}

// Direct conversion for convenience in code that uses TopicPatternPath::new_from_string
impl From<TopicPatternError> for MqttClientError {
	fn from(err: TopicPatternError) -> Self {
		MqttClientError::Topic(TopicError::Pattern(err))
	}
}

impl From<SendError<SubscriptionId>> for MqttClientError {
	fn from(SendError(sub_id): SendError<SubscriptionId>) -> Self {
		MqttClientError::UnsubscribeFailed(sub_id)
	}
}

impl From<std::convert::Infallible> for MqttClientError {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
}