use rumqttc::{ClientError, OptionError};
use tokio::sync::mpsc::error::SendError;

use crate::{
	routing::SubscriptionError,
	topic::{
		SubscriptionId, TopicError, TopicPatternError,
		topic_pattern_path::TopicFormatError,
	},
};

/// Errors that can occur during MQTT connection establishment phase.
///
/// These errors represent different failure modes when attempting to establish
/// an initial connection to the MQTT broker. They are distinguished from runtime
/// connection errors that occur after a successful initial connection.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionEstablishmentError {
	/// Network-level connection failure (DNS, TCP, TLS, etc.)
	///
	/// This variant wraps underlying network errors from rumqttc, such as:
	/// - DNS resolution failures
	/// - TCP connection timeouts
	/// - TLS handshake failures
	/// - Invalid URLs or connection parameters
	#[error("Network connection failed: {0}")]
	Network(Box<rumqttc::ConnectionError>),

	/// MQTT broker rejected the connection attempt
	///
	/// The network connection was successful, but the MQTT broker refused
	/// the connection for protocol-level reasons (authentication, protocol version, etc.)
	#[error("Broker rejected connection: {code:?}")]
	BrokerRejected {
		/// The specific rejection reason code from the broker
		code: rumqttc::ConnectReturnCode,
	},

	/// Connection establishment exceeded the configured timeout
	///
	/// The connection attempt took longer than the specified timeout period.
	/// This can happen due to network latency, broker overload, or network instability.
	#[error("Connection establishment timed out after {timeout_millis}ms")]
	Timeout {
		/// The timeout duration that was exceeded, in milliseconds
		timeout_millis: u64,
	},
}

impl From<rumqttc::ConnectionError> for ConnectionEstablishmentError {
	fn from(err: rumqttc::ConnectionError) -> Self {
		Self::Network(Box::new(err))
	}
}

/// Errors that can occur in MQTT client operations
#[derive(Debug, thiserror::Error)]
pub enum MqttClientError {
	/// Connection-related errors from rumqttc
	#[error("Client operation failed: {0}")]
	ClientOperation(#[from] ClientError),

	/// Configuration errors when parsing MQTT options
	#[error("Configuration error: {0}")]
	Configuration(#[from] OptionError),

	/// Invalid configuration parameter values
	#[error("Invalid configuration value: {0}")]
	ConfigurationValue(String),

	/// Serialization errors when converting data to bytes
	#[error("Serialization error: {0}")]
	Serialization(String),

	/// Subscription management errors
	#[error("Subscription error: {0}")]
	Subscription(#[from] SubscriptionError),
	/// Topic format errors
	#[error("Topic format error: {0}")]
	TopicFormat(#[from] TopicFormatError),

	/// Topic pattern errors  
	#[error("Topic pattern error: {0}")]
	TopicPattern(#[from] TopicPatternError),

	/// Topic-related errors (pattern, matching, routing)
	#[error("Topic error: {0}")]
	Topic(#[from] TopicError),

	/// Channel communication errors
	#[error("Failed to unsubscribe: subscription {0} channel closed")]
	UnsubscribeFailed(SubscriptionId),

	/// Connection establishment failed
	#[error("Failed to establish connection: {0}")]
	ConnectionEstablishment(#[from] ConnectionEstablishmentError),
}

impl MqttClientError {
	/// Create a TopicPattern error
	pub fn topic_pattern(err: TopicPatternError) -> Self {
		MqttClientError::TopicPattern(err) // 🔄 ЗМІНЕНО: тепер пряма конверсія
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
