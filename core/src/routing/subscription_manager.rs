#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
use std::{
	collections::{HashMap, VecDeque},
	num::NonZeroUsize,
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::Duration,
};

use arcstr::ArcStr;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use lru::LruCache;
use mqtt_topic_engine::QoS;
use rumqttc::AsyncClient;
use tokio::{
	sync::{
		mpsc::{
			self as tokio_mpsc, Receiver, Sender, channel,
			error::{SendTimeoutError, TrySendError},
		},
		oneshot,
	},
	task::{JoinError, JoinHandle},
};
use tracing::{debug, error, info, warn};

use super::error::{SendError, SubscriptionError};
use super::subscriber::Subscriber;
use crate::message_meta::{MessageMeta, RawMeta};
use crate::topic::{
	SubscriptionId, TopicPatternPath, TopicRouter,
	topic_match::{TopicMatch, TopicPath},
};

pub type RawMessageType<T> = (String, RawMeta, T);
pub type MessageType<T> = (Arc<TopicMatch>, Arc<MessageMeta>, Arc<T>);

/// Default per-subscriber channel capacity (buffered messages before backpressure).
const DEFAULT_CHANNEL_CAPACITY: NonZeroUsize =
	NonZeroUsize::new(500).expect("500 is non-zero");
/// Default grace period a message may wait for a slow consumer before it is dropped.
const DEFAULT_SLOW_SEND_TIMEOUT: Duration = Duration::from_secs(2);
/// Default number of messages that may queue behind an in-flight slow send.
const DEFAULT_MAX_PARKED_MESSAGES: usize = 100;

type TopicRouterType<T> = TopicRouter<SubscriberEntry<T>>;

/// Per-subscription delivery configuration.
///
/// Controls both the broker-facing QoS and this client's local backpressure
/// policy. Incoming messages for one subscription are delivered to the
/// subscriber's channel in the order the broker sent them; the fields below
/// govern the **buffer → grace → drop** pipeline that protects the routing
/// actor from a slow consumer:
///
/// 1. **Buffer** — each subscriber has a bounded channel of
///    [`channel_capacity`](Self::channel_capacity) messages. Fast delivery goes
///    straight into it.
/// 2. **Grace** — when the channel is full, the next message is *parked*: a
///    single in-flight send waits up to [`slow_send_timeout`](Self::slow_send_timeout)
///    for the consumer to free capacity. Further messages for that subscriber
///    queue **behind** the parked one (up to
///    [`max_parked_messages`](Self::max_parked_messages)), preserving order.
/// 3. **Drop** — if the grace period elapses, or the parked queue overflows,
///    messages are dropped (never reordered). Every drop increments a
///    per-subscriber counter exposed via `dropped_messages()`.
///
/// This type is `#[non_exhaustive]`: MQTT 5 will add subscription options
/// (e.g. `no_local`, `retain_handling`) additively. Construct it via
/// [`SubscriptionConfig::default`] and adjust fields, or use the builder
/// methods on the subscription builder.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SubscriptionConfig {
	/// Broker-facing subscription QoS.
	pub qos: QoS,
	/// Bounded capacity of the subscriber's delivery channel.
	///
	/// Practically bounded by available memory; absurdly large values (near
	/// `usize::MAX`) exceed the channel implementation's limit and are invalid.
	pub channel_capacity: NonZeroUsize,
	/// How long a parked message waits for a slow consumer before being dropped.
	pub slow_send_timeout: Duration,
	/// How many messages may queue behind an in-flight slow send before the
	/// newest incoming message is dropped.
	pub max_parked_messages: usize,
}

impl Default for SubscriptionConfig {
	fn default() -> Self {
		Self {
			qos: QoS::AtLeastOnce,
			channel_capacity: DEFAULT_CHANNEL_CAPACITY,
			slow_send_timeout: DEFAULT_SLOW_SEND_TIMEOUT,
			max_parked_messages: DEFAULT_MAX_PARKED_MESSAGES,
		}
	}
}

/// A subscribe request. Boxed inside [`Command`] so the large subscribe payload
/// does not bloat every queued `Send` (the hot per-message path).
#[derive(Debug)]
pub struct SubscribeRequest<T> {
	topic: TopicPatternPath,
	config: SubscriptionConfig,
	response_tx: oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
}

#[derive(Debug)]
pub enum Command<T> {
	Subscribe(Box<SubscribeRequest<T>>),
	Send(RawMessageType<T>),
	ResubscribeAll(oneshot::Sender<Result<(), SubscriptionError>>),
	/// Terminal: break the actor loop and run cleanup. Sent by the event-loop
	/// task when it dies (any reason), so subscriber channels are closed and
	/// `receive()` yields `None` instead of hanging — the same cleanup the
	/// controller's `shutdown_tx` path triggers on `MqttConnection::shutdown()`.
	Shutdown,
}

/// Router payload for one subscription: the delivery channel plus the state the
/// routing actor needs to enforce the backpressure policy.
struct SubscriberEntry<T> {
	sender: Sender<MessageType<T>>,
	dropped_messages: Arc<AtomicU64>,
	slow_send_timeout: Duration,
	max_parked_messages: usize,
}

// Manual `Clone`: a derive would demand `T: Clone`, but none of the fields hold
// a bare `T` (the channel carries `Arc<T>`), so the entry is always cloneable.
impl<T> Clone for SubscriberEntry<T> {
	fn clone(&self) -> Self {
		Self {
			sender: self.sender.clone(),
			dropped_messages: Arc::clone(&self.dropped_messages),
			slow_send_timeout: self.slow_send_timeout,
			max_parked_messages: self.max_parked_messages,
		}
	}
}

impl<T> SubscriberEntry<T> {
	fn record_drop(&self, count: u64) {
		self.dropped_messages.fetch_add(count, Ordering::Relaxed);
	}
}

/// Messages waiting behind the single in-flight slow send for one subscriber.
///
/// Invariant maintained by the actor: `parked` contains an entry for a
/// subscription **if and only if** exactly one slow-send future for it is
/// in flight. The queue holds the messages that must be delivered *after* that
/// send completes, so per-subscriber order is never violated.
struct ParkedState<T> {
	queue: VecDeque<MessageType<T>>,
	entry: SubscriberEntry<T>,
}

type SlowSendResult<T> =
	(SubscriptionId, Result<(), SendTimeoutError<MessageType<T>>>);

pub struct SubscriptionManagerActor<T> {
	topic_router: TopicRouterType<T>,
	/// LRU cache for TopicPath instances to avoid repeated parsing of topic strings.
	/// Used in single-threaded actor context - no synchronization needed here.
	topic_path_cache: LruCache<ArcStr, Arc<TopicPath>>,
	client: AsyncClient,
	command_rx: Receiver<Command<T>>,
	unsubscribe_tx: Sender<SubscriptionId>,
	unsubscribe_rx: Receiver<SubscriptionId>,
	shutdown_rx: oneshot::Receiver<()>,
	/// At most one pending send per slow subscriber; the set is naturally
	/// bounded by the number of active subscriptions.
	slow_send_futures: FuturesUnordered<BoxFuture<'static, SlowSendResult<T>>>,
	/// Messages queued behind an in-flight slow send, keyed by subscription.
	parked: HashMap<SubscriptionId, ParkedState<T>>,
}

impl<T> SubscriptionManagerActor<T>
where T: Send + Sync + 'static
{
	pub fn spawn(
		client: AsyncClient,
		topic_path_cache_capacity: NonZeroUsize,
		command_channel_capacity: usize,
		unsubscribe_channel_capacity: usize,
	) -> (SubscriptionManagerController, SubscriptionManagerHandler<T>) {
		let (command_tx, command_rx) = channel(command_channel_capacity);
		let (unsubscribe_tx, unsubscribe_rx) =
			channel(unsubscribe_channel_capacity);
		let (shutdown_tx, shutdown_rx) = oneshot::channel();
		let actor = Self {
			topic_router: TopicRouterType::<T>::new(),
			topic_path_cache: LruCache::new(topic_path_cache_capacity),
			client,
			command_rx,
			unsubscribe_tx,
			unsubscribe_rx,
			shutdown_rx,
			slow_send_futures: FuturesUnordered::new(),
			parked: HashMap::new(),
		};
		let join_handler = tokio::spawn(async move { actor.run().await });

		let controller = SubscriptionManagerController {
			shutdown_tx,
			join_handler,
		};
		let handler = SubscriptionManagerHandler { command_tx };

		(controller, handler)
	}

	async fn run(mut self) {
		loop {
			tokio::select! {
				_ = &mut self.shutdown_rx => {
				info!("SubscriptionManagerActor: Shutdown signal received");
				break;
				}
				Some(slow_send_res) = self.slow_send_futures.next() => {
					self.handle_slow_send(slow_send_res).await;
				}
				cmd = self.command_rx.recv() => {
					if let Some(cmd) = cmd {
						match cmd {
						Command::Send(message) => self.handle_send(message).await,
						Command::Subscribe(req) => {
							let SubscribeRequest { topic, config, response_tx } = *req;
							self.handle_subscribe(topic, config, response_tx).await
						},
						Command::ResubscribeAll(response_tx) => {
							self.handle_resubscribe_all(response_tx).await;
						}
						Command::Shutdown => {
							info!("SubscriptionManagerActor: Shutdown command received");
							break;
						}
					}
					} else {
						info!("SubscriptionManagerActor: Command channel closed, exiting");
						break;
					}
				}
				subs_id = self.unsubscribe_rx.recv() => {
					if let Some(subs_id) = subs_id {
						self.handle_unsubscribe(&subs_id).await;
					} else {
						// NOTE: This should never happen since actor holds unsubscribe_tx
						// But keeping for defensive programming
						warn!("Unsubscribe channel unexpectedly closed");
					}
				}
			}
		}
		info!("SubscriptionManagerActor: Exiting run loop");
		// Cleanup remaining subscriptions
		self.cleanup_active_subscriptions().await
	}

	/// Handle completion of the single in-flight slow send for a subscriber.
	///
	/// On success or timeout the parked queue is drained in order; a closed
	/// channel tears the subscription down. A completion for a subscription that
	/// was already removed (e.g. unsubscribed while the send was in flight) is a
	/// no-op — the `parked` map is the source of truth for the invariant.
	async fn handle_slow_send(&mut self, (id, result): SlowSendResult<T>) {
		if !self.parked.contains_key(&id) {
			debug!(
				subscription_id = ?id,
				"Slow send completed for a no-longer-parked subscription; ignoring"
			);
			return;
		}
		match result {
			| Ok(()) => {}
			| Err(SendTimeoutError::Timeout(msg)) => {
				if let Some(state) = self.parked.get(&id) {
					state.entry.record_drop(1);
				}
				error!(
					subscription_id = ?id,
					topic = %msg.0,
					"Slow send timeout for subscriber. message dropped",
				);
			}
			| Err(SendTimeoutError::Closed(msg)) => {
				if let Some(state) = self.parked.remove(&id) {
					// The failed head plus everything queued behind it is lost.
					state.entry.record_drop(1 + state.queue.len() as u64);
				}
				error!(
					subscription_id = ?id,
					topic = %msg.0,
					"slow_send channel closed, message dropped. unsubscribing",
				);
				self.handle_unsubscribe(&id).await;
				return;
			}
		}
		self.drain_parked(&id).await;
	}

	/// Deliver messages queued behind a just-completed slow send, in order.
	///
	/// Stops (re-parking) at the first message the channel still cannot accept,
	/// keeping exactly one send in flight. Removes the `parked` entry once the
	/// queue is empty, restoring the fast path for that subscriber.
	async fn drain_parked(&mut self, id: &SubscriptionId) {
		// Take ownership so we can freely touch other `self` fields while
		// draining; re-insert only if the subscriber is still behind.
		let Some(mut state) = self.parked.remove(id) else {
			return;
		};
		while let Some(msg) = state.queue.pop_front() {
			match state.entry.sender.try_send(msg) {
				| Ok(()) => continue,
				| Err(TrySendError::Full(msg)) => {
					let entry = state.entry.clone();
					self.parked.insert(*id, state);
					self.push_slow_send(entry, *id, msg);
					return;
				}
				| Err(TrySendError::Closed(_)) => {
					// Dropped head + everything still queued.
					state.entry.record_drop(1 + state.queue.len() as u64);
					error!(
						subscription_id = ?id,
						"Subscriber channel closed while draining. unsubscribing",
					);
					self.handle_unsubscribe(id).await;
					return;
				}
			}
		}
		// Queue emptied: the subscriber has caught up, fast path resumes.
	}

	/// Push a bounded, grace-period send for `msg` into the slow-send set.
	///
	/// The future is not spawned as a task: it lives in `slow_send_futures` and
	/// is polled by the actor loop, so it cannot outlive the actor and its
	/// `(id, result)` outcome can never be lost to a `JoinError`.
	fn push_slow_send(
		&mut self,
		entry: SubscriberEntry<T>,
		id: SubscriptionId,
		msg: MessageType<T>,
	) {
		let fut = async move {
			let result = entry
				.sender
				.send_timeout(msg, entry.slow_send_timeout)
				.await;
			(id, result)
		};
		self.slow_send_futures.push(Box::pin(fut));
	}

	/// Cleanup all active subscriptions and resources during shutdown
	/// Order of operations is important for graceful cleanup:
	/// 1. Send unsubscribe commands to MQTT broker for all topics
	/// 2. Drop parked queues so in-flight sends stop re-parking during the grace
	/// 3. Process remaining slow sends with timeout
	/// 4. Cleanup internal data structures
	async fn cleanup_active_subscriptions(&mut self) {
		// Step 1: Send unsubscribe commands to MQTT broker for all active topics
		// This prevents new messages from being received
		let active_subscriptions =
			self.topic_router.get_topics_for_unsubscribe();

		for mqtt_topic in active_subscriptions {
			if let Err(err) = self.client.unsubscribe(mqtt_topic.as_str()).await
			{
				error!(
					topic_pattern = %mqtt_topic,
					error = ?err,
					"Failed to unsubscribe from topic pattern"
				);
			}
		}

		// Step 2: Drop everything still queued behind slow sends. This clears
		// the `parked` invariant so the in-flight sends drained below simply
		// complete instead of re-parking fresh (doomed) sends during the grace.
		let dropped_parked: u64 =
			self.parked.drain().map(|(_, s)| s.queue.len() as u64).sum();
		if dropped_parked > 0 {
			warn!(
				dropped_messages = dropped_parked,
				"Dropping parked messages during shutdown"
			);
		}

		// Step 3: Process any remaining slow sends with a timeout
		// This ensures we don't wait indefinitely for slow subscribers
		let process_slow_sends = async {
			while let Some(slow_send_res) = self.slow_send_futures.next().await
			{
				self.handle_slow_send(slow_send_res).await;
			}
		};
		let res = tokio::time::timeout(
			Duration::from_millis(500),
			process_slow_sends,
		)
		.await;
		let _ = res.inspect_err(|_| {
			warn!(
				timeout_ms = 500,
				"SubscriptionManagerActor: Cleanup slow_send timeout"
			);
		});

		// Step 4: Cleanup internal topic router data structures
		// This closes all subscriber channels and clears state
		self.topic_router.cleanup();
	}

	async fn handle_subscribe(
		&mut self,
		topic: TopicPatternPath,
		config: SubscriptionConfig,
		response_tx: oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
	) {
		let (channel_tx, channel_rx) =
			tokio_mpsc::channel(config.channel_capacity.get());
		let dropped_messages = Arc::new(AtomicU64::new(0));
		let entry = SubscriberEntry {
			sender: channel_tx,
			dropped_messages: Arc::clone(&dropped_messages),
			slow_send_timeout: config.slow_send_timeout,
			max_parked_messages: config.max_parked_messages,
		};
		let topic_patern_str = topic.mqtt_pattern();
		let (needs_subscribe, id) =
			self.topic_router.add_subscription(topic, config.qos, entry);
		if needs_subscribe {
			let res = self
				.client
				.subscribe(topic_patern_str.as_str(), config.qos.to_rumqttc())
				.await;
			if let Err(err) = res {
				if let Err(unsub_err) = self.topic_router.unsubscribe(&id) {
					warn!(
						subscription_id = ?id,
						error = ?unsub_err,
						"Failed to cleanup subscription after subscribe error"
					);
				}
				error!(
					topic = %topic_patern_str,
					error = ?err,
					"Failed to subscribe to MQTT topic"
				);
				if response_tx
					.send(Err(SubscriptionError::SubscribeFailed))
					.is_err()
				{
					warn!(
						topic = %topic_patern_str,
						"Could not send subscribe error response (channel full/closed)"
					);
				}
				return;
			}
		}
		let subscriber = Subscriber::new(
			channel_rx,
			self.unsubscribe_tx.clone(),
			id,
			dropped_messages,
		);
		if response_tx.send(Ok(subscriber)).is_err() {
			warn!(
				subscription_id = ?id,
				"Could not send successful subscribe response (channel full/closed)"
			);
			self.handle_unsubscribe(&id).await;
		}
	}

	async fn handle_resubscribe_all(
		&mut self,
		response_tx: oneshot::Sender<Result<(), SubscriptionError>>,
	) {
		let subscriptions = self.topic_router.get_topics_for_resubscribe();
		let mut failed_topics = Vec::new();

		for (mqtt_topic, qos) in subscriptions {
			match self
				.client
				.subscribe(mqtt_topic.as_str(), qos.to_rumqttc())
				.await
			{
				| Ok(_) => {
					debug!(topic = %mqtt_topic, qos = ?qos, "Successfully resubscribed");
				}
				| Err(err) => {
					error!(topic = %mqtt_topic, qos = ?qos, error = ?err, "Failed to resubscribe");
					failed_topics.push(mqtt_topic);
				}
			}
		}

		let result = if failed_topics.is_empty() {
			Ok(())
		} else {
			Err(SubscriptionError::ResubscribeFailed)
		};

		let _ = response_tx.send(result).inspect_err(|_| {
			warn!("Could not send resubscribe response (channel full/closed)");
		});
	}

	async fn handle_unsubscribe(&mut self, id: &SubscriptionId) {
		// Drop any messages parked for this subscription regardless of the
		// router outcome (keeps the `parked` invariant and prevents leaks).
		if let Some(state) = self.parked.remove(id) {
			if !state.queue.is_empty() {
				debug!(
					subscription_id = ?id,
					dropped_messages = state.queue.len(),
					"Dropping parked messages on unsubscribe"
				);
			}
		}
		match self.topic_router.unsubscribe(id) {
			| Ok((topic_empty, topic_pattern)) => {
				if topic_empty {
					let res = self
						.client
						.unsubscribe(topic_pattern.mqtt_pattern().as_str())
						.await;
					if let Err(err) = res {
						error!(
							topic_pattern = %topic_pattern,
							error = ?err,
							"Failed to unsubscribe from MQTT topic pattern"
						);
					}
					debug!(topic_pattern = %topic_pattern, "Topic pattern now empty");
				}
			}
			| Err(err) => {
				error!(subscription_id = ?id, error = ?err, "Failed to unsubscribe");
			}
		}
	}

	async fn handle_send(
		&mut self,
		(topic_str, raw_meta, data): RawMessageType<T>,
	) {
		let topic_arcstr = ArcStr::from(topic_str);
		// First level cache: TopicPath creation from string (this actor's cache)
		let topic_path = match self.topic_path_cache.get(&topic_arcstr) {
			| Some(path) => path.clone(),
			| None => {
				let path = Arc::new(TopicPath::new(topic_arcstr.clone()));
				self.topic_path_cache.put(topic_arcstr, path.clone());
				path
			}
		};

		// Second level cache: TopicMatch results are cached inside TopicPatternPath (per-pattern cache)

		let subscribers = self.topic_router.get_subscribers(&topic_path);
		let mut closed_subscribers = Vec::new();
		// Subscribers that just overflowed and need a fresh slow send. Collected
		// here and started after the loop, once the `topic_router` borrow ends.
		let mut new_parks: Vec<(
			SubscriptionId,
			SubscriberEntry<T>,
			MessageType<T>,
		)> = Vec::new();
		let data = Arc::new(data);
		// Built once per publish and shared (like the payload Arc) across every
		// subscriber of this topic.
		let meta = Arc::new(MessageMeta::new(
			raw_meta.qos,
			raw_meta.retain,
			raw_meta.dup,
		));
		for (id, (topic_patern, _qos), entry) in subscribers {
			let topic_match =
				match topic_patern.try_match(Arc::clone(&topic_path)) {
					| Ok(match_result) => match_result,
					| Err(err) => {
						error!(
							subscription_id = ?id,
							topic = %topic_path,
							error = ?err,
							"Failed to match topic pattern"
						);
						continue;
					}
				};
			let message = (topic_match, Arc::clone(&meta), Arc::clone(&data));

			// A slow send is already in flight for this subscriber: queue behind
			// it to preserve order instead of racing it via try_send.
			if let Some(state) = self.parked.get_mut(id) {
				if state.queue.len() >= entry.max_parked_messages {
					entry.record_drop(1);
					warn!(
						subscription_id = ?id,
						topic = %topic_path,
						max_parked = entry.max_parked_messages,
						"Parked queue full. Message dropped",
					);
				} else {
					state.queue.push_back(message);
				}
				continue;
			}

			match entry.sender.try_send(message) {
				| Ok(_) => (),
				| Err(TrySendError::Closed(_)) => {
					closed_subscribers.push(*id);
				}
				| Err(TrySendError::Full(msg)) => {
					new_parks.push((*id, entry.clone(), msg));
				}
			}
		}
		// `subscribers` (and the `topic_router` borrow) end here.
		for (id, entry, msg) in new_parks {
			self.parked.insert(
				id,
				ParkedState {
					queue: VecDeque::new(),
					entry: entry.clone(),
				},
			);
			self.push_slow_send(entry, id, msg);
		}
		for closed_id in closed_subscribers {
			self.handle_unsubscribe(&closed_id).await
		}
	}
}

pub struct SubscriptionManagerController {
	shutdown_tx: oneshot::Sender<()>,
	join_handler: JoinHandle<()>,
}

impl SubscriptionManagerController {
	pub async fn shutdown(self) -> Result<(), JoinError> {
		let _ = self.shutdown_tx.send(()).inspect_err(|_| {
			warn!(
				"SubscriptionManagerController: Shutdown signal already sent"
			);
		});
		self.join_handler.await.inspect_err(|e| {
			warn!(
				error = ?e,
				"SubscriptionManagerController: Actor run failed"
			);
		})
	}
}

#[derive(Clone, Debug)]
pub struct SubscriptionManagerHandler<T> {
	command_tx: Sender<Command<T>>,
}

impl<T> SubscriptionManagerHandler<T>
where T: Send + Sync + 'static
{
	pub(crate) async fn create_subscription(
		&self,
		topic: TopicPatternPath,
		config: SubscriptionConfig,
	) -> Result<Subscriber<T>, SubscriptionError> {
		let (tx, rx) = oneshot::channel();
		self.command_tx
			.send(Command::Subscribe(Box::new(SubscribeRequest {
				topic,
				config,
				response_tx: tx,
			})))
			.await
			.map_err(|_| SubscriptionError::ChannelClosed)?;
		rx.await.map_err(|_| SubscriptionError::ResponseLost)?
	}

	pub(crate) async fn resubscribe_all(
		&self,
	) -> Result<(), SubscriptionError> {
		let (tx, rx) = oneshot::channel();
		self.command_tx
			.send(Command::ResubscribeAll(tx))
			.await
			.map_err(|_| SubscriptionError::ChannelClosed)?;
		rx.await.map_err(|_| SubscriptionError::ResponseLost)?
	}

	pub(crate) async fn dispatch_incoming_message(
		&self,
		topic: String,
		meta: RawMeta,
		data: T,
	) -> Result<(), SendError> {
		self.command_tx
			.send(Command::Send((topic, meta, data)))
			.await
			.map_err(|_| SendError::ChannelClosed)
	}

	/// Tell the actor to break its loop and run subscription cleanup.
	///
	/// Best-effort: a closed command channel means the actor already exited
	/// (e.g. via the controller's `shutdown_tx` on `MqttConnection::shutdown()`),
	/// so the send error is expected and ignored. Called by the event-loop task
	/// on terminal death so consumers get `None` from `receive()`.
	pub(crate) async fn shutdown(&self) {
		let _ = self.command_tx.send(Command::Shutdown).await;
	}
}

#[cfg(test)]
mod tests {
	use rumqttc::MqttOptions;

	use super::*;

	const PATTERN: &str = "test/topic";

	/// Builds an actor whose channels are inert: the command/unsubscribe/shutdown
	/// ends we don't drive are dropped, and the backend event loop is discarded
	/// (the client is never exercised by these tests). Subscriptions are wired
	/// directly through the router so no broker interaction is needed.
	fn make_actor<T: Send + Sync + 'static>() -> SubscriptionManagerActor<T> {
		let (client, _eventloop) =
			AsyncClient::new(MqttOptions::new("test", "localhost", 1883), 10);
		let (_command_tx, command_rx) = channel(10);
		let (unsubscribe_tx, unsubscribe_rx) = channel(10);
		let (_shutdown_tx, shutdown_rx) = oneshot::channel();
		SubscriptionManagerActor {
			topic_router: TopicRouterType::new(),
			topic_path_cache: LruCache::new(
				NonZeroUsize::new(10).expect("10 is non-zero"),
			),
			client,
			command_rx,
			unsubscribe_tx,
			unsubscribe_rx,
			shutdown_rx,
			slow_send_futures: FuturesUnordered::new(),
			parked: HashMap::new(),
		}
	}

	/// Like `make_actor`, but hands back the LIVE command + shutdown senders so a
	/// test can spawn the real `run()` select loop and drive it through the
	/// command channel. Keeping `shutdown_tx` alive is essential: a dropped
	/// oneshot sender makes the `&mut self.shutdown_rx` select arm fire
	/// immediately, which would exit the loop via the controller path and mask
	/// the `Command::Shutdown` path under test.
	fn make_actor_with_channels<T: Send + Sync + 'static>() -> (
		SubscriptionManagerActor<T>,
		Sender<Command<T>>,
		oneshot::Sender<()>,
	) {
		let (client, _eventloop) =
			AsyncClient::new(MqttOptions::new("test", "localhost", 1883), 10);
		let (command_tx, command_rx) = channel(10);
		let (unsubscribe_tx, unsubscribe_rx) = channel(10);
		let (shutdown_tx, shutdown_rx) = oneshot::channel();
		let actor = SubscriptionManagerActor {
			topic_router: TopicRouterType::new(),
			topic_path_cache: LruCache::new(
				NonZeroUsize::new(10).expect("10 is non-zero"),
			),
			client,
			command_rx,
			unsubscribe_tx,
			unsubscribe_rx,
			shutdown_rx,
			slow_send_futures: FuturesUnordered::new(),
			parked: HashMap::new(),
		};
		(actor, command_tx, shutdown_tx)
	}

	// The terminal `Command::Shutdown` must break the actor loop and run cleanup,
	// closing each subscriber's delivery channel so `receive()` yields `None`
	// (the zombie-consumer fix). Without cleanup the actor would run forever and
	// the consumer would park on an empty-but-open channel.
	#[tokio::test]
	async fn shutdown_command_closes_subscriber_channels() {
		let (mut actor, command_tx, _shutdown_tx) =
			make_actor_with_channels::<String>();
		let (_id, mut rx, _dropped) =
			add_sub(&mut actor, 1, Duration::from_secs(5), 10);

		let handle = tokio::spawn(actor.run());

		command_tx
			.send(Command::Shutdown)
			.await
			.expect("actor alive to receive shutdown");

		handle.await.expect("actor task joins after shutdown");
		assert!(
			rx.recv().await.is_none(),
			"delivery channel must be closed after cleanup"
		);
	}

	/// Registers a subscription directly on the router and returns its id, the
	/// receiving end of its delivery channel, and its drop counter.
	fn add_sub(
		actor: &mut SubscriptionManagerActor<String>,
		capacity: usize,
		timeout: Duration,
		max_parked: usize,
	) -> (
		SubscriptionId,
		Receiver<MessageType<String>>,
		Arc<AtomicU64>,
	) {
		let (tx, rx) = tokio_mpsc::channel(capacity);
		let dropped = Arc::new(AtomicU64::new(0));
		let entry = SubscriberEntry {
			sender: tx,
			dropped_messages: Arc::clone(&dropped),
			slow_send_timeout: timeout,
			max_parked_messages: max_parked,
		};
		let pattern =
			TopicPatternPath::try_from(PATTERN).expect("valid literal pattern");
		let (_needs, id) = actor.topic_router.add_subscription(
			pattern,
			QoS::AtLeastOnce,
			entry,
		);
		(id, rx, dropped)
	}

	async fn send(actor: &mut SubscriptionManagerActor<String>, payload: &str) {
		let meta = RawMeta {
			qos: QoS::AtLeastOnce,
			retain: false,
			dup: false,
		};
		actor
			.handle_send((PATTERN.to_string(), meta, payload.to_string()))
			.await;
	}

	/// Polls the single pending slow send to completion and processes it,
	/// mirroring the actor's `run` loop for one slow-send tick.
	async fn pump_slow_send(actor: &mut SubscriptionManagerActor<String>) {
		let res = actor
			.slow_send_futures
			.next()
			.await
			.expect("a slow send is pending");
		actor.handle_slow_send(res).await;
	}

	fn payload(msg: &MessageType<String>) -> &str {
		msg.2.as_str()
	}

	// A slow consumer must never receive messages out of order, even though the
	// blocked message is handed off to an async slow-send task.
	#[tokio::test]
	async fn slow_consumer_preserves_order() {
		let mut actor = make_actor::<String>();
		let (_id, mut rx, dropped) =
			add_sub(&mut actor, 1, Duration::from_secs(5), 10);

		send(&mut actor, "A").await; // fills the capacity-1 channel
		send(&mut actor, "B").await; // channel full -> parked (slow send in flight)
		send(&mut actor, "C").await; // queues behind B

		// Free capacity by consuming A, then let B's slow send complete.
		assert_eq!(payload(&rx.recv().await.unwrap()), "A");
		pump_slow_send(&mut actor).await; // B delivered; C re-parked behind it
		assert_eq!(payload(&rx.recv().await.unwrap()), "B");
		pump_slow_send(&mut actor).await; // C delivered
		assert_eq!(payload(&rx.recv().await.unwrap()), "C");

		assert_eq!(dropped.load(Ordering::Relaxed), 0);
		assert!(actor.parked.is_empty());
	}

	// Once the parked queue is full, further incoming messages are dropped
	// (newest first) and counted; delivered messages keep their order.
	#[tokio::test]
	async fn parked_overflow_drops_newest() {
		let mut actor = make_actor::<String>();
		// capacity 1, room for 2 parked messages behind the in-flight send.
		let (_id, mut rx, dropped) =
			add_sub(&mut actor, 1, Duration::from_secs(5), 2);

		send(&mut actor, "A").await; // fills channel
		send(&mut actor, "B").await; // parked (in flight)
		send(&mut actor, "C").await; // queue[0]
		send(&mut actor, "D").await; // queue[1] (queue now full)
		send(&mut actor, "E").await; // dropped (queue full)

		assert_eq!(dropped.load(Ordering::Relaxed), 1);

		assert_eq!(payload(&rx.recv().await.unwrap()), "A");
		pump_slow_send(&mut actor).await;
		assert_eq!(payload(&rx.recv().await.unwrap()), "B");
		pump_slow_send(&mut actor).await;
		assert_eq!(payload(&rx.recv().await.unwrap()), "C");
		pump_slow_send(&mut actor).await;
		assert_eq!(payload(&rx.recv().await.unwrap()), "D");

		assert_eq!(dropped.load(Ordering::Relaxed), 1); // still just E
	}

	// A grace-period timeout drops the parked head; the rest still arrives in
	// order once the consumer catches up.
	#[tokio::test]
	async fn timeout_drops_head_preserves_rest() {
		let mut actor = make_actor::<String>();
		let (_id, mut rx, dropped) =
			add_sub(&mut actor, 1, Duration::from_millis(50), 10);

		send(&mut actor, "A").await; // fills channel, never consumed yet
		send(&mut actor, "B").await; // parked (in flight, 50ms grace)
		send(&mut actor, "C").await; // queues behind B

		// Do not consume A: B's slow send times out and is dropped, then C is
		// re-parked (channel still full of A).
		pump_slow_send(&mut actor).await;
		assert_eq!(dropped.load(Ordering::Relaxed), 1); // B dropped

		// Now drain A, let C through.
		assert_eq!(payload(&rx.recv().await.unwrap()), "A");
		pump_slow_send(&mut actor).await; // C delivered
		assert_eq!(payload(&rx.recv().await.unwrap()), "C");

		assert_eq!(dropped.load(Ordering::Relaxed), 1); // only B was lost
		assert!(actor.parked.is_empty());
	}

	// A subscriber whose channel closes while a send is parked has its whole
	// queue dropped (and counted) and is unsubscribed.
	#[tokio::test]
	async fn closed_channel_drops_queue_and_unsubscribes() {
		let mut actor = make_actor::<String>();
		let (id, rx, dropped) =
			add_sub(&mut actor, 1, Duration::from_secs(5), 10);

		send(&mut actor, "A").await; // fills channel
		send(&mut actor, "B").await; // parked (in flight)
		send(&mut actor, "C").await; // queues behind B

		drop(rx); // consumer goes away

		pump_slow_send(&mut actor).await; // B's send sees a closed channel

		// B (the in-flight head) + C (queued) are both dropped.
		assert_eq!(dropped.load(Ordering::Relaxed), 2);
		assert!(actor.parked.is_empty());
		assert!(actor.topic_router.get_topic_by_id(&id).is_err());
	}

	// With drops recorded while a message sits buffered, the low-level subscriber
	// surfaces them as a `Lagged` event ahead of that message, and advances its
	// watermark so each burst is reported exactly once (no phantom repeat).
	#[tokio::test]
	async fn subscriber_recv_surfaces_lag_before_messages() {
		use crate::ReceiveEvent;

		let mut actor = make_actor::<String>();
		let (id, rx, dropped) =
			add_sub(&mut actor, 4, Duration::from_secs(5), 10);
		let mut subscriber = Subscriber::new(
			rx,
			actor.unsubscribe_tx.clone(),
			id,
			Arc::clone(&dropped),
		);

		send(&mut actor, "A").await; // buffered, ready to deliver
		dropped.fetch_add(3, Ordering::Relaxed); // simulate 3 backpressure drops

		// Lag is reported first, before the buffered message.
		assert!(matches!(
			subscriber.recv().await,
			Some(ReceiveEvent::Lagged { missed: 3 })
		));
		match subscriber.recv().await {
			| Some(ReceiveEvent::Message((_topic, _meta, payload))) => {
				assert_eq!(payload.as_str(), "A")
			}
			| _ => panic!("expected the buffered message A"),
		}

		// A fresh burst is reported once more; the previous delta is not repeated.
		dropped.fetch_add(2, Ordering::Relaxed);
		send(&mut actor, "B").await;
		assert!(matches!(
			subscriber.recv().await,
			Some(ReceiveEvent::Lagged { missed: 2 })
		));
		match subscriber.recv().await {
			| Some(ReceiveEvent::Message((_topic, _meta, payload))) => {
				assert_eq!(payload.as_str(), "B")
			}
			| _ => panic!("expected the buffered message B"),
		}
	}
}
