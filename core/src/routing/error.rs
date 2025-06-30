
/// Errors when sending messages through channels
#[derive(Debug)]
pub enum SendError {
	/// Channel has been closed
	ChannelClosed,
}

/// Errors during subscription operations
#[derive(Debug)]
pub enum SubscriptionError {
	/// Communication channel closed
	ChannelClosed,
	/// Response from subscription manager was lost
	ResponseLost,
	/// Failed to subscribe to MQTT broker
	SubscribeFailed,
}
