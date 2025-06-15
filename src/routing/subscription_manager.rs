#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
use std::{collections::HashMap, sync::Arc, time::Duration};

use arcstr::ArcStr;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
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
use crate::topic::{
	SubscriptionId, TopicPatternPath, TopicRouter,
	topic_match::{TopicMatch, TopicPath},
};

pub type RawMessageType<T> = (String, T);
pub type MessageType<T> = (TopicMatch, Arc<T>);

type TopicRouterType<T> = TopicRouter<Sender<MessageType<T>>>;

#[derive(Debug)]
pub struct SubscriptionConfig {
	pub qos: rumqttc::QoS,
}

impl Default for SubscriptionConfig {
	fn default() -> Self {
		Self {
			qos: rumqttc::QoS::AtLeastOnce,
		}
	}
}

#[derive(Debug)]
pub enum Command<T> {
	Unsubscribe(SubscriptionId),
	Subscribe(
		TopicPatternPath,
		SubscriptionConfig,
		oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
	),
	Send(RawMessageType<T>),
}

type SlowSendResult<T> = (
	SubscriptionId,
	Result<(), tokio_mpsc::error::SendTimeoutError<MessageType<T>>>,
);

pub struct SubscriptionManagerActor<T> {
	topic_router: TopicRouterType<T>,
	topic_path_cache: HashMap<String, TopicPath>,
	client: AsyncClient,
	command_rx: Receiver<Command<T>>,
	command_tx: Sender<Command<T>>, //For unsubscribe message
	shutdown_rx: oneshot::Receiver<()>,
	slow_send_futures:
		FuturesUnordered<tokio::task::JoinHandle<SlowSendResult<T>>>,
}

impl<T> SubscriptionManagerActor<T>
where T: Send + Sync + 'static
{
	pub fn spawn(
		client: AsyncClient,
	) -> (SubscriptionManagerController, SubscriptionManagerHandler<T>) {
		let (command_tx, command_rx) = channel(100);
		let (shutdown_tx, shutdown_rx) = oneshot::channel();
		let actor = Self {
			topic_router: TopicRouterType::<T>::new(),
			topic_path_cache: HashMap::new(),
			client,
			command_rx,
			command_tx: command_tx.clone(),
			shutdown_rx,
			slow_send_futures: FuturesUnordered::new(),
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
							Command::Unsubscribe(id) => self.handle_unsubscribe(&id).await,
							Command::Send(message) => self.handle_send(message).await,
							Command::Subscribe(topic, config, response_tx) => {
								self.handle_subscribe(topic, config, response_tx).await
							}
						}
						} else {
							info!("SubscriptionManagerActor: Command channel closed, exiting");
							break;
						}
					}
				}
		}
		info!("SubscriptionManagerActor: Exiting run loop");
		// Cleanup remaining subscriptions
		self.cleanup_active_subscriptions().await
	}

	async fn handle_slow_send(
		&mut self,
		slow_send_res: Result<SlowSendResult<T>, JoinError>,
	) {
		match slow_send_res {
			| Ok((_, Ok(()))) => {}
			| Ok((subs_id, Err(SendTimeoutError::Closed(msg)))) => {
				self.handle_unsubscribe(&subs_id).await;
				error!(
					subscription_id = ?subs_id,
					topic = %msg.0,
					"slow_send channel closed, message dropped. unsubscribing",
				);
			}
			| Ok((subs_id, Err(SendTimeoutError::Timeout(msg)))) => {
				error!(
					subscription_id = ?subs_id,
					topic = %msg.0,
					"Slow send timeout for subscriber. message dropped",
				);
			}
			| Err(err) => {
				error!(error = ?err, "Failed to complete slow_send task");
			}
		}
	}
	/// Cleanup all active subscriptions and resources during shutdown
	/// Order of operations is important for graceful cleanup:
	/// 1. Send unsubscribe commands to MQTT broker for all topics
	/// 2. Process remaining slow sends with timeout
	/// 3. Cleanup internal data structures
	async fn cleanup_active_subscriptions(&mut self) {
		// Step 1: Send unsubscribe commands to MQTT broker for all active topics
		// This prevents new messages from being received
		let active_subscriptions = self.topic_router.get_active_subscriptions();

		for topic in active_subscriptions {
			if let Err(err) = self.client.unsubscribe(topic.to_string()).await {
				error!(
					topic_pattern = %topic,
					error = ?err,
					"Failed to unsubscribe from topic pattern"
				);
			}
		}

		// Step 2: Process any remaining slow sends with a timeout
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

		// Step 3: Cleanup internal topic router data structures
		// This closes all subscriber channels and clears state
		self.topic_router.cleanup();
	}

	async fn handle_subscribe(
		&mut self,
		topic: TopicPatternPath,
		config: SubscriptionConfig,
		response_tx: oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
	) {
		let (channel_tx, channel_rx) = tokio_mpsc::channel(500);
		let topic_patern_str = topic.to_mqtt_subscription_pattern();
		let (fresh_topic, id) = self.topic_router.subscribe(topic, channel_tx);
		if fresh_topic {
			let res = self
				.client
				.subscribe(topic_patern_str.clone(), config.qos)
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
		let subscriber =
			Subscriber::new(channel_rx, self.command_tx.clone(), id);
		if response_tx.send(Ok(subscriber)).is_err() {
			warn!(
				subscription_id = ?id,
				"Could not send successful subscribe response (channel full/closed)"
			);
			self.handle_unsubscribe(&id).await;
		}
	}

	async fn handle_unsubscribe(&mut self, id: &SubscriptionId) {
		match self.topic_router.unsubscribe(id) {
			| Ok((topic_empty, topic_pattern)) => {
				if topic_empty {
					let res = self
						.client
						.unsubscribe(topic_pattern.to_string())
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

	async fn handle_send(&mut self, (topic, data): RawMessageType<T>) {
		let topic = ArcStr::from(topic);
		//TODO Create cache for TopicPath to avoid re-parsing
		let topic = TopicPath::new(topic);
		let subscribers = self.topic_router.get_subscribers(&topic);
		let mut closed_subscribers = Vec::new();
		let data = Arc::new(data);
		subscribers.iter().for_each(|(id, topic_patern, sender)| {
			let topic_match = match topic_patern.try_match(topic.clone()) {
				| Ok(match_result) => match_result,
				| Err(err) => {
					error!(
						subscription_id = ?id,
						topic = %topic,
						error = ?err,
						"Failed to match topic pattern"
					);
					return;
				}
			};
			match sender.try_send((topic_match, Arc::clone(&data))) {
				| Ok(_) => (),
				| Err(TrySendError::Closed(_)) => {
					closed_subscribers.push(**id);
				}
				| Err(TrySendError::Full(msg)) => {
					if self.slow_send_futures.len() >= 100 {
						error!(
							subscription_id = ?id,
							topic = %topic,
							queue_size = self.slow_send_futures.len(),
							"Too many slow sends in processing queue. Message dropped",
						);
						return;
					}
					let sender_clone = (*sender).clone();
					let id_clone = **id;

					let slow_send_handle = tokio::spawn(async move {
						let send_result = sender_clone
							.send_timeout(msg, Duration::from_secs(2))
							.await;
						(id_clone, send_result)
					});
					self.slow_send_futures.push(slow_send_handle);
				}
			}
		});
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

#[derive(Clone)]
pub struct SubscriptionManagerHandler<T> {
	command_tx: Sender<Command<T>>,
}

impl<T> SubscriptionManagerHandler<T>
where T: Send + Sync + 'static
{
	pub async fn subscribe(
		&self,
		topic: TopicPatternPath,
		config: SubscriptionConfig,
	) -> Result<Subscriber<T>, SubscriptionError> {
		let (tx, rx) = oneshot::channel();
		self.command_tx
			.send(Command::Subscribe(topic, config, tx))
			.await
			.map_err(|_| SubscriptionError::ChannelClosed)?;
		rx.await.map_err(|_| SubscriptionError::ResponseLost)?
	}

	pub async fn send_data(
		&self,
		topic: String,
		data: T,
	) -> Result<(), SendError> {
		self.command_tx
			.send(Command::Send((topic, data)))
			.await
			.map_err(|_| SendError::ChannelClosed)
	}
}
