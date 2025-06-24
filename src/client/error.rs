use rumqttc::{ClientError, OptionError};

use crate::{TopicPatternError, TopicRouterError, routing::SubscriptionError};

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
	/// Invalid topic pattern errors
	TopicPattern(TopicPatternError),
	/// Topic routing errors
	TopicRouting(TopicRouterError),
	/// Channel communication errors
	Channel(String),
}

impl std::fmt::Display for MqttClientError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			| MqttClientError::Connection(e) => {
				write!(f, "Connection error: {}", e)
			}
			| MqttClientError::Configuration(e) => {
				write!(f, "Configuration error: {}", e)
			}
			| MqttClientError::ConfigurationValue(e) => {
				write!(f, "Invalid configuration value: {}", e)
			}
			| MqttClientError::Serialization(e) => {
				write!(f, "Serialization error: {}", e)
			}
			| MqttClientError::Subscription(e) => {
				write!(f, "Subscription error: {:?}", e)
			}
			| MqttClientError::TopicPattern(e) => {
				write!(f, "Topic pattern error: {}", e)
			}
			| MqttClientError::TopicRouting(e) => {
				write!(f, "Topic routing error: {}", e)
			}
			| MqttClientError::Channel(e) => write!(f, "Channel error: {}", e),
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

impl From<TopicPatternError> for MqttClientError {
	fn from(err: TopicPatternError) -> Self {
		MqttClientError::TopicPattern(err)
	}
}

impl From<TopicRouterError> for MqttClientError {
	fn from(err: TopicRouterError) -> Self {
		MqttClientError::TopicRouting(err)
	}
}
