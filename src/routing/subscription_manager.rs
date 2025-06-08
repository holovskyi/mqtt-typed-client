use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use rumqttc::AsyncClient;
use string_cache::DefaultAtom as Topic;
use tokio::{
	sync::{
		mpsc::{
			self as tokio_mpsc, Receiver, Sender, channel, error::TrySendError,
		},
		oneshot,
	},
	task::{JoinError, JoinHandle},
};

use super::error::{SendError, SubscriptionError};
use super::subscriber::Subscriber;
use crate::topic::{SubscriptionId, TopicPatternPath, TopicRouter};

pub type MessageType<T> = (Topic, Arc<T>);
type TopicRouterType<T> = TopicRouter<Sender<MessageType<T>>>;

#[derive(Debug)]
pub enum Command<T> {
	Unsubscribe(SubscriptionId),
	Subscribe(
		TopicPatternPath,
		oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
	),
	Send(MessageType<T>),
}

type SlowSendResult<T> = (
	SubscriptionId,
	Result<(), tokio_mpsc::error::SendError<(Topic, Arc<T>)>>,
);

pub struct SubscriptionManagerActor<T> {
	topic_router: TopicRouterType<T>,
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
					eprintln!("SubscriptionManagerActor: Shutdown signal received.");
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
						Command::Subscribe(topic, response_tx) => {
							self.handle_subscribe(topic, response_tx).await
						}
					}
					} else {
						eprintln!("SubscriptionManagerActor: Command channel closed, exiting.");
						break;
					}
				}
			}
		}
		eprintln!("SubscriptionManagerActor: Exiting run loop.");
		// Cleanup remaining subscriptions
		self.cleanup_active_subscriptions().await
	}

	async fn handle_slow_send(
		&mut self,
		slow_send_res: Result<SlowSendResult<T>, JoinError>,
	) {
		match slow_send_res {
			| Ok((_, Ok(()))) => {}
			| Ok((subs_id, Err(send_res))) => {
				self.handle_unsubscribe(&subs_id).await;
				eprintln!(
					"Warning: Slow subscriber {subs_id:?} disconnected: \
					 {send_res}"
				);
			}
			| Err(err) => {
				eprintln!("Error: Cant finish slow_send {err}");
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
				eprintln!(
					"Error: Can't unsubscribe from pattern {:?}. Error:{err}",
					topic
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
		let _ = res.inspect_err(|err| {
			eprintln!(
				"Warning: SubscriptionManagerActor: Cleanup slow_send \
				 timeout: {err}"
			);
		});

		// Step 3: Cleanup internal topic router data structures
		// This closes all subscriber channels and clears state
		self.topic_router.cleanup();
	}

	async fn handle_subscribe(
		&mut self,
		topic: TopicPatternPath,
		response_tx: oneshot::Sender<Result<Subscriber<T>, SubscriptionError>>,
	) {
		let (channel_tx, channel_rx) = tokio_mpsc::channel(500);
		let topic_clone = topic.to_string();
		let (fresh_topic, id) = self.topic_router.subscribe(topic, channel_tx);
		if fresh_topic {
			//TODO what about QOS configure
			let res = self
				.client
				.subscribe(topic_clone.clone(), rumqttc::QoS::AtLeastOnce)
				.await;
			if let Err(err) = res {
				if let Err(unsub_err) = self.topic_router.unsubscribe(&id) {
					eprintln!(
						"Warning: Failed to cleanup subscription {id:?}: \
						 {unsub_err}"
					);
				}
				eprintln!(
					"Error: can't subscribe to topic {topic_clone} Error:{err}"
				);
				if response_tx
					.send(Err(SubscriptionError::SubscribeFailed))
					.is_err()
				{
					eprintln!(
						"Warning: Could not send subscribe response for \
						 {topic_clone} (channel full/closed)"
					);
				}
				return;
			}
		}
		let subscriber =
			Subscriber::new(channel_rx, self.command_tx.clone(), id);
		if response_tx.send(Ok(subscriber)).is_err() {
			eprintln!(
				"Warning: Could not Subscribe {id:#?} (channel full/closed)",
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
						eprintln!(
							"Error: Can't unsubscribe from pattern {:?}. \
							 Error:{err}",
							topic_pattern
						);
					}
					eprintln!("Topic pattern {:?} now empty", topic_pattern);
				}
			}
			| Err(err) => {
				eprintln!("Error: Failed to unsubscribe {:?}: {}", id, err);
			}
		}
	}

	async fn handle_send(&mut self, (topic, data): MessageType<T>) {
		let subscribers = self.topic_router.get_subscribers(&topic);
		let mut closed_subscribers = Vec::new();
		subscribers.iter().for_each(|(id, sender)| {
			match sender.try_send((topic.clone(), Arc::clone(&data))) {
				| Ok(_) => (),
				| Err(TrySendError::Closed(_)) => {
					closed_subscribers.push(**id);
				}
				| Err(TrySendError::Full(msg)) => {
					if self.slow_send_futures.len() >= 100 {
						eprintln!(
							"Warning: Too many slow sends in processing \
							 queue: {topic}"
						);
					}
					let sender_clone = (*sender).clone();
					let id_clone = **id;

					let slow_send_handle = tokio::spawn(async move {
						let send_res = sender_clone.send(msg).await;
						(id_clone, send_res)
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
			eprintln!(
				"Warning: SubscriptionManagerController: Shutdown signal \
				 already sent."
			);
		});
		self.join_handler.await.inspect_err(|e| {
			eprintln!(
				"Warning: SubscriptionManagerController: Actor run failed \
				 with error: {e}"
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
	) -> Result<Subscriber<T>, SubscriptionError> {
		let (tx, rx) = oneshot::channel();
		self.command_tx
			.send(Command::Subscribe(topic, tx))
			.await
			.map_err(|_| SubscriptionError::ChannelClosed)?;
		rx.await.map_err(|_| SubscriptionError::ResponseLost)?
	}

	pub async fn send_data(
		&self,
		topic: Topic,
		data: T,
	) -> Result<(), SendError> {
		self.command_tx
			.send(Command::Send((topic, Arc::new(data))))
			.await
			.map_err(|_| SendError::ChannelClosed)
	}
}
