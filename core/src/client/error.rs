use tokio::sync::mpsc::error::SendError;

use crate::{
	routing::SubscriptionError,
	topic::{
		SubscriptionId, TopicError, TopicPatternError,
		topic_pattern_path::TopicFormatError,
	},
};

/// Reason a broker rejected (or accepted) a connection attempt.
///
/// Modeled as the MQTT 5 CONNACK reason-code superset; MQTT 3.1.1 return
/// codes map into the matching subset. Codes that can only be produced by an
/// MQTT 5 broker are documented as such and are never emitted in 0.3.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReasonCode {
	/// Connection accepted
	Success,
	/// The broker does not wish to reveal the reason (MQTT 5 only)
	UnspecifiedError,
	/// CONNECT packet could not be parsed correctly (MQTT 5 only)
	MalformedPacket,
	/// Data in the CONNECT does not conform to the specification (MQTT 5 only)
	ProtocolError,
	/// Implementation-specific rejection (MQTT 5 only)
	ImplementationSpecificError,
	/// Broker does not support the requested protocol version
	UnsupportedProtocolVersion,
	/// Client identifier is valid UTF-8 but not allowed by the broker
	ClientIdentifierNotValid,
	/// Username or password rejected
	BadUserNamePassword,
	/// Client is not authorized to connect
	NotAuthorized,
	/// MQTT service is unavailable
	ServerUnavailable,
	/// Broker is busy, try again later (MQTT 5 only)
	ServerBusy,
	/// Client has been banned by administrative action (MQTT 5 only)
	Banned,
	/// Authentication method is not supported (MQTT 5 only)
	BadAuthenticationMethod,
	/// Will topic name is not accepted (MQTT 5 only)
	TopicNameInvalid,
	/// CONNECT packet exceeded broker's maximum packet size (MQTT 5 only)
	PacketTooLarge,
	/// Implementation or administrative quota exceeded (MQTT 5 only)
	QuotaExceeded,
	/// Will payload does not match its payload format indicator (MQTT 5 only)
	PayloadFormatInvalid,
	/// Broker does not support retained messages (MQTT 5 only)
	RetainNotSupported,
	/// Will QoS is higher than the broker supports (MQTT 5 only)
	QoSNotSupported,
	/// Client should temporarily use another server (MQTT 5 only)
	UseAnotherServer,
	/// Client should permanently use another server (MQTT 5 only)
	ServerMoved,
	/// Connection rate limit exceeded (MQTT 5 only)
	ConnectionRateExceeded,
}

impl ConnectReasonCode {
	pub(crate) fn from_v4(code: rumqttc::ConnectReturnCode) -> Self {
		use rumqttc::ConnectReturnCode as V4;
		match code {
			| V4::Success => Self::Success,
			| V4::RefusedProtocolVersion => Self::UnsupportedProtocolVersion,
			| V4::BadClientId => Self::ClientIdentifierNotValid,
			| V4::ServiceUnavailable => Self::ServerUnavailable,
			| V4::BadUserNamePassword => Self::BadUserNamePassword,
			| V4::NotAuthorized => Self::NotAuthorized,
		}
	}
}

/// Opaque error from the underlying MQTT backend.
///
/// Preserves the full `Display` message and `source()` chain of the backend
/// error without exposing the backend crate's types in our public API.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct BackendError(Box<dyn std::error::Error + Send + Sync>);

impl BackendError {
	pub(crate) fn new(
		err: impl std::error::Error + Send + Sync + 'static,
	) -> Self {
		Self(Box::new(err))
	}
}

/// Errors when handing an operation to the MQTT client's request queue.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ClientOperationError {
	/// The request channel to the event loop is closed (client shut down)
	#[error("MQTT request channel closed")]
	RequestChannelClosed,
	/// The request channel to the event loop is full (backpressure)
	#[error("MQTT request channel full")]
	RequestChannelFull,
}

impl ClientOperationError {
	// rumqttc's `ClientError::TryRequest` has already discarded the
	// Full/Disconnected discriminant, so `TryRequest -> RequestChannelFull`
	// is an approximation. We never call `try_*` methods today, so the
	// variant is currently unreachable.
	pub(crate) fn from_backend(err: rumqttc::ClientError) -> Self {
		match err {
			| rumqttc::ClientError::Request(_) => Self::RequestChannelClosed,
			| rumqttc::ClientError::TryRequest(_) => Self::RequestChannelFull,
		}
	}
}

/// Errors when parsing an MQTT connection URL.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum UrlParseError {
	/// The string is not a valid URL
	#[error("invalid URL: {0}")]
	Invalid(String),
	/// Unsupported URL scheme (expected tcp/mqtt/ssl/mqtts/ws/wss)
	#[error("unsupported URL scheme `{0}`")]
	Scheme(String),
	/// The URL has no host
	#[error("URL is missing a host")]
	MissingHost,
	/// The required `client_id` query parameter is missing
	#[error("URL is missing the `client_id` query parameter")]
	MissingClientId,
	/// A query parameter has an invalid value
	#[error("invalid `{name}` query parameter: {reason}")]
	InvalidParam {
		/// Parameter name
		name: String,
		/// Why the value was rejected
		reason: String,
	},
	/// A backend-tuning parameter that is intentionally not part of the URL
	/// grammar; configure it via the `unstable-backend-api` escape hatch.
	#[error(
		"query parameter `{0}` is not supported; configure it via the \
		 `unstable-backend-api` escape hatch instead"
	)]
	UnsupportedParam(String),
	/// An unrecognized query parameter
	#[error("unknown query parameter `{0}`")]
	UnknownParam(String),
	/// Unsupported `protocol=` value
	#[error("unsupported protocol version `{0}` (expected `4` or `5`)")]
	UnsupportedProtocol(String),
}

/// Errors that can occur during MQTT connection establishment phase.
///
/// These errors represent different failure modes when attempting to establish
/// an initial connection to the MQTT broker. They are distinguished from runtime
/// connection errors that occur after a successful initial connection.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ConnectionEstablishmentError {
	/// Network-level connection failure (DNS, TCP, TLS, I/O, protocol state)
	#[error("Network connection failed: {0}")]
	Network(#[source] BackendError),

	/// MQTT broker rejected the connection attempt
	///
	/// The network connection was successful, but the MQTT broker refused
	/// the connection for protocol-level reasons (authentication, protocol version, etc.)
	#[error("Broker rejected connection: {code:?}")]
	BrokerRejected {
		/// The specific rejection reason code from the broker
		code: ConnectReasonCode,
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

impl ConnectionEstablishmentError {
	// Broker rejection travels as `ConnectionError::ConnectionRefused` on some
	// code paths; normalize it so a rejection has exactly one representation.
	pub(crate) fn from_backend(err: rumqttc::ConnectionError) -> Self {
		match err {
			| rumqttc::ConnectionError::ConnectionRefused(code) => {
				Self::BrokerRejected {
					code: ConnectReasonCode::from_v4(code),
				}
			}
			| other => Self::Network(BackendError::new(other)),
		}
	}
}

/// Errors that can occur in MQTT client operations
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum MqttClientError {
	/// Failed to hand an operation to the MQTT request queue
	#[error("Client operation failed: {0}")]
	ClientOperation(#[from] ClientOperationError),

	/// Errors when parsing an MQTT connection URL
	#[error("Configuration error: {0}")]
	Configuration(#[from] UrlParseError),

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
		MqttClientError::TopicPattern(err)
	}

	pub(crate) fn from_backend_client_error(err: rumqttc::ClientError) -> Self {
		Self::ClientOperation(ClientOperationError::from_backend(err))
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
