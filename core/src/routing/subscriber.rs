use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc::{
	Receiver, Sender,
	error::{SendError, TrySendError},
};
use tracing::{debug, warn};

use crate::{
	routing::subscription_manager::MessageType, topic::SubscriptionId,
};

/// Outcome of a single `receive()`/`recv()` call.
///
/// `None` from the receiving method still means the subscription is closed;
/// this enum describes the *non-terminal* outcomes of a call that produced
/// something. Lagging is modeled as an **event**, not an error: the stream is
/// healthy and the next buffered message is intact — you were merely told that
/// some earlier messages were dropped.
///
/// `#[non_exhaustive]`: future kinds of stream event may be added without a
/// breaking change, so external `match`es need a wildcard arm.
///
/// # Consuming — do NOT `while let Some(Message(..))`
///
/// A `DecodeFailed` or `Lagged` event refutes a
/// `while let Some(ReceiveEvent::Message(m)) = sub.receive().await` pattern and
/// **silently ends the loop** — under load `Lagged` is common, so that shape
/// turns backpressure into a dead consumer. Match every variant:
///
/// ```ignore
/// while let Some(event) = sub.receive().await {
///     match event {
///         ReceiveEvent::Message(m) => { /* handle */ }
///         ReceiveEvent::DecodeFailed(e) => { /* log */ }
///         ReceiveEvent::Lagged { missed } => { /* warn: dropped `missed` */ }
///         _ => {}
///     }
/// }
/// ```
///
/// or opt out explicitly with [`message`](Self::message) (problems are already
/// logged by the library):
///
/// ```ignore
/// while let Some(event) = sub.receive().await {
///     let Some(m) = event.message() else { continue };
///     // handle m
/// }
/// ```
#[non_exhaustive]
#[derive(Debug)]
pub enum ReceiveEvent<M, E> {
	/// The next message.
	Message(M),
	/// A message arrived but could not be decoded. The stream continues.
	DecodeFailed(E),
	/// `missed` messages destined for this subscriber were dropped by the
	/// backpressure policy (see [`SubscriptionConfig`](crate::SubscriptionConfig))
	/// since the previous report.
	///
	/// Positional note: `missed` is an exact count, but its position in the
	/// stream is approximate. The notice is surfaced as soon as `recv()` observes
	/// the drop, which can be *before* messages that were still buffered from
	/// ahead of the gap — so treat `missed` as "this many messages were lost
	/// recently", not an exact marker of where the gap sits.
	Lagged {
		/// Number of messages dropped since the last `Lagged` event.
		missed: u64,
	},
}

impl<M, E> ReceiveEvent<M, E> {
	/// Explicit opt-out that keeps only messages, discarding decode failures and
	/// lag notices. Greppable, unlike a silent `while let Some(Message(_))`.
	pub fn message(self) -> Option<M> {
		match self {
			| ReceiveEvent::Message(m) => Some(m),
			| _ => None,
		}
	}
}

/// Low-level MQTT message subscriber with manual unsubscription
#[derive(Debug)]
pub struct Subscriber<T> {
	receiver: Receiver<MessageType<T>>,
	unsubscribe_tx: Option<Sender<SubscriptionId>>,
	id: SubscriptionId,
	dropped_messages: Arc<AtomicU64>,
	/// Cumulative drop count already reported to the consumer via `Lagged`.
	last_seen_drops: u64,
}

impl<T> Subscriber<T> {
	/// Creates a new subscriber with the given channels.
	pub(crate) fn new(
		receiver: Receiver<MessageType<T>>,
		unsubscribe_tx: Sender<SubscriptionId>,
		id: SubscriptionId,
		dropped_messages: Arc<AtomicU64>,
	) -> Self {
		Self {
			receiver,
			unsubscribe_tx: Some(unsubscribe_tx),
			id,
			dropped_messages,
			last_seen_drops: 0,
		}
	}

	/// Receives the next stream event from the subscription.
	///
	/// Returns `None` once the subscription is closed. A pending drop is surfaced
	/// as [`ReceiveEvent::Lagged`] as soon as this call observes the drop counter,
	/// which may be *ahead of* messages still buffered from before the gap — so
	/// `missed` is an exact count but only an approximate stream position (see
	/// [`ReceiveEvent::Lagged`]). Drops recorded after the consumer has already
	/// parked on a channel that then closes may only be observable via
	/// [`dropped_messages`](Self::dropped_messages).
	pub async fn recv(
		&mut self,
	) -> Option<ReceiveEvent<MessageType<T>, Infallible>> {
		if let Some(missed) = self.take_lag() {
			return Some(ReceiveEvent::Lagged { missed });
		}
		self.receiver.recv().await.map(ReceiveEvent::Message)
	}

	/// Returns the number of newly-dropped messages since the last report, if
	/// any, advancing the local watermark. `dropped_messages` is monotonic
	/// (`fetch_add` only), so the subtraction never underflows.
	fn take_lag(&mut self) -> Option<u64> {
		let seen = self.dropped_messages.load(Ordering::Relaxed);
		let missed = seen - self.last_seen_drops;
		if missed > 0 {
			self.last_seen_drops = seen;
			Some(missed)
		} else {
			None
		}
	}

	/// Cumulative number of messages dropped for this subscription because the
	/// consumer could not keep up (grace-period timeouts and parked-queue
	/// overflow). This is the metrics side channel; per-event notification is
	/// [`ReceiveEvent::Lagged`].
	///
	/// A steadily rising count signals this subscriber is too slow for its
	/// incoming rate; raise `channel_capacity` / `max_parked_messages` or
	/// consume faster. See [`SubscriptionConfig`](crate::SubscriptionConfig).
	pub fn dropped_messages(&self) -> u64 {
		self.dropped_messages.load(Ordering::Relaxed)
	}

	/// Unsubscribes from the topic pattern.
	pub async fn unsubscribe(
		mut self,
	) -> Result<(), SendError<SubscriptionId>> {
		if let Some(unsubscribe_tx) = self.unsubscribe_tx.take() {
			unsubscribe_tx.send(self.id).await
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
