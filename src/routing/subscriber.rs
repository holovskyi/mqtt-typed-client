use tokio::sync::mpsc::{Receiver, Sender, error::TrySendError};
use tracing::{debug, warn};

use super::subscription_manager::{Command, MessageType};
use crate::topic::SubscriptionId;

#[derive(Debug)]
pub struct Subscriber<T> {
	receiver: Receiver<MessageType<T>>,
	//TODO chhange cancel_tx on abstract interface for canceling subscription
	cancel_tx: Option<Sender<Command<T>>>,
	id: SubscriptionId,
}

impl<T> Subscriber<T> {
	pub fn new(
		receiver: Receiver<MessageType<T>>,
		cancel_tx: Sender<Command<T>>,
		id: SubscriptionId,
	) -> Self {
		Self {
			receiver,
			cancel_tx: Some(cancel_tx),
			id,
		}
	}

	pub async fn recv(&mut self) -> Option<MessageType<T>> {
		self.receiver.recv().await
	}
	//TODO map error to custom error
	pub async fn unsubscribe(mut self) -> Result<(), String> {
		if let Some(cancel_tx) = self.cancel_tx.take() {
			cancel_tx
				.send(Command::Unsubscribe(self.id))
				.await
				.map_err(|_| "Can't send unsubscribe command".to_string())
		} else {
			warn!(subscription_id = ?self.id, "Subscription already canceled");
			Ok(())
		}
	}

	pub fn unsubscribe_immediate(
		&mut self,
	) -> Result<(), TrySendError<Command<T>>> {
		if let Some(cancel_tx) = self.cancel_tx.take() {
			let err = cancel_tx.try_send(Command::Unsubscribe(self.id));
			if err.is_err() {
				self.cancel_tx = Some(cancel_tx);
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
		if let Some(cancel_tx) = self.cancel_tx.take() {
			match cancel_tx.try_send(Command::Unsubscribe(self.id)) {
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
				| Err(err) => warn!(
					subscription_id = ?self.id,
					error = ?err,
					"Failed to unsubscribe in Drop"
				),
			}
		}
	}
}
