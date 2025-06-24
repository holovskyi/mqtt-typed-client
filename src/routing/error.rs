
#[derive(Debug)]
pub enum SendError {
	ChannelClosed,
}

#[derive(Debug)]
pub enum SubscriptionError {
	ChannelClosed,
	ResponseLost,
	SubscribeFailed,
}
