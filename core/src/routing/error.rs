use thiserror::Error;

/// Errors when sending messages through channels
#[derive(Debug, Error)]
pub enum SendError {
	/// Channel has been closed
	#[error("Channel has been closed")]
	ChannelClosed,
}

/// Errors during subscription operations
#[derive(Debug, Error)]
pub enum SubscriptionError {
	/// Communication channel closed
	#[error("Communication channel closed")]
	ChannelClosed,
	/// Response from subscription manager was lost
	#[error("Response from subscription manager was lost")]
	ResponseLost,
	/// Failed to subscribe to MQTT broker
	#[error("Failed to subscribe to MQTT broker")]
	SubscribeFailed,
	/// Failed to resubscribe to topics after reconnect
	#[error("Failed to resubscribe after reconnect")]
	ResubscribeFailed,
}
