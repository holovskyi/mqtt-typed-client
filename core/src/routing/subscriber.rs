use tokio::sync::mpsc::{error::{SendError, TrySendError}, Receiver, Sender};
use tracing::{debug, warn};

use crate::{
	routing::subscription_manager::MessageType, topic::SubscriptionId,
};

/// Low-level MQTT message subscriber with manual unsubscription
#[derive(Debug)]
pub struct Subscriber<T> {
	receiver: Receiver<MessageType<T>>,
	unsubscribe_tx: Option<Sender<SubscriptionId>>,
	id: SubscriptionId,
}

impl<T> Subscriber<T> {
	/// Creates a new subscriber with the given channels.
	pub fn new(
		receiver: Receiver<MessageType<T>>,
		unsubscribe_tx: Sender<SubscriptionId>,
		id: SubscriptionId,
	) -> Self {
		Self {
			receiver,
			unsubscribe_tx: Some(unsubscribe_tx),
			id,
		}
	}

	/// Receives the next message from subscription.
	pub async fn recv(&mut self) -> Option<MessageType<T>> {
		self.receiver.recv().await
	}

	/// Unsubscribes from the topic pattern.
	pub async fn unsubscribe(mut self) -> Result<(), SendError<SubscriptionId>> {
		if let Some(unsubscribe_tx) = self.unsubscribe_tx.take() {
			unsubscribe_tx
				.send(self.id)
				.await
		} else {
			warn!(subscription_id = ?self.id, "Subscription already canceled");
			Ok(())
		}
	}

	/// Immediately unsubscribes without waiting.
	pub fn unsubscribe_immediate(
		&mut self,
	) -> Result<(), TrySendError<SubscriptionId>> {
		if let Some(unsubscribe_tx) = self.unsubscribe_tx.take() {
			let err = unsubscribe_tx.try_send(self.id);
			if err.is_err() {
				self.unsubscribe_tx = Some(unsubscribe_tx);
				warn!(
					subscription_id = ?self.id,
					"Failed to send unsubscribe command"
				);
			}
			err
		} else {
			warn!(subscription_id = ?self.id, "Subscription already canceled");
			Ok(())
		}
	}
}

impl<T> Drop for Subscriber<T> {
	fn drop(&mut self) {
		if let Some(unsubscribe_tx) = self.unsubscribe_tx.take() {
			match unsubscribe_tx.try_send(self.id) {
				| Ok(_) => {
					debug!(
						subscription_id = ?self.id,
						"Subscription unsubscribed in Drop"
					);
				}
				| Err(TrySendError::Closed(_)) => {
					// The channel is closed, meaning the subscription manager has already
					// processed the unsubscribe command, so we can just ignore this.
				}
				| Err(err) => {
					warn!(
						subscription_id = ?self.id,
						error = ?err,
							"Failed to unsubscribe in Drop"
					);
				}
			}
		}
	}
}
