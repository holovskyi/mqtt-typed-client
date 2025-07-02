use std::marker::PhantomData;

use arcstr::ArcStr;

use crate::{
	structured::FromMqttMessage, MessageSerializer, MqttClient, MqttClientError, MqttTopicSubscriber, SubscriptionConfig, TopicPatternError, TopicPatternPath
};

/// Immutable builder for configuring MQTT subscriptions
#[derive(Debug)]
pub struct SubscriptionBuilder<MessageType, F:Clone> {
	client: MqttClient<F>,
	pattern: TopicPatternPath,
	config: SubscriptionConfig,
	_phantom: PhantomData<MessageType>,
}

impl<MessageType, F> Clone for SubscriptionBuilder<MessageType, F>
where F: Clone
{
	fn clone(&self) -> Self {
		Self {
			pattern: self.pattern.clone(),
			config: self.config.clone(),
			client: self.client.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<MessageType, F> SubscriptionBuilder<MessageType, F>
where F:Clone {
	/// Create new builder with default pattern
	pub fn new(
		client: MqttClient<F>,
		default_pattern: TopicPatternPath,
	) -> Self {
		Self {
			client,
			pattern: default_pattern,
			config: SubscriptionConfig::default(),
			_phantom: PhantomData,
		}
	}

	/// Add value for topic wildcard parameter
	#[doc(hidden)]
	pub fn bind_parameter(
		mut self,
		param_name: impl Into<ArcStr>,
		value: impl Into<ArcStr>,
	) -> Result<Self, TopicPatternError> {
		self.pattern = self.pattern.bind_parameter(param_name, value)?;
		Ok(self)
	}

	/// Set cache capacity
	pub fn with_cache(self, capacity: usize) -> Self {
		Self {
			pattern: self
				.pattern
				.with_cache_strategy(crate::CacheStrategy::new(capacity)),
			..self
		}
	}

	/// Set QoS level
	pub fn with_qos(self, qos: rumqttc::QoS) -> Self {
		Self {
			config: SubscriptionConfig { qos },
			..self
		}
	}

	/// Replace pattern while preserving wildcard structure
	pub fn with_pattern(
		self,
		custom_pattern: impl TryInto<TopicPatternPath, Error: Into<MqttClientError>>,
	) -> Result<Self, MqttClientError> {
		let new_pattern = custom_pattern.try_into().map_err(Into::into)?;
		let validated_pattern =
			self.pattern.check_pattern_compatibility(new_pattern)?;

		Ok(Self {
			pattern: validated_pattern,
			..self
		})
	}

	/// Subscribe using configured parameters
	pub async fn subscribe<PayloadType>(
		self,
	) -> Result<MqttTopicSubscriber<MessageType, PayloadType, F>, MqttClientError>
	where
		MessageType: FromMqttMessage<PayloadType, F::DeserializeError>,
		PayloadType: Send + Sync + 'static,
		F: Default + Clone + Send + Sync + MessageSerializer<PayloadType>,
	{
		let subscriber = self
			.client
			.subscribe_with_config(self.pattern, self.config)
			.await?;
		Ok(MqttTopicSubscriber::new(subscriber))
	}
}
