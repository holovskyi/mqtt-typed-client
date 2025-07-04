use rumqttc::{ClientError, OptionError};
use tokio::sync::mpsc::error::SendError;

use crate::{
	routing::SubscriptionError, 
	topic::{topic_pattern_path::TopicFormatError, SubscriptionId, TopicError, TopicPatternError}
};

#[derive(Debug, thiserror::Error)]
pub enum ConnectionEstablishmentError {
    #[error("Network connection failed: {0}")]
    Network(#[from] rumqttc::ConnectionError),
    
    #[error("Broker rejected connection: {code:?}")]
    BrokerRejected { code: rumqttc::ConnectReturnCode },
    
    #[error("Connection establishment timed out after {timeout_millis}ms")]
    Timeout { timeout_millis : u64 },
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