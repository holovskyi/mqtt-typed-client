#[derive(Debug)]
pub enum SendError {
	ChannelClosed,
	//InvalidTopic(String),
}

#[derive(Debug)]
pub enum SubscriptionError {
	ChannelClosed,
	ResponseLost,
	SubscribeFailed,
}
