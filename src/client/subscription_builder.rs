use std::marker::PhantomData;

use crate::{
	MqttClient, MqttClientError, MqttTopicSubscriber,
	SubscriptionConfig, TopicPatternPath, MessageSerializer,
	structured::FromMqttMessage,
};

/// Immutable builder for configuring MQTT subscriptions
#[derive(Debug)]
pub struct SubscriptionBuilder<MessageType> {
	pattern: TopicPatternPath,
	config: SubscriptionConfig,
	_phantom: PhantomData<MessageType>,
}

impl<MessageType> Clone for SubscriptionBuilder<MessageType> {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            config: self.config.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<MessageType> SubscriptionBuilder<MessageType> {
	/// Create new builder with default pattern
	pub fn new(default_pattern: TopicPatternPath) -> Self {
		Self {
			pattern: default_pattern,
			config: SubscriptionConfig::default(),
			_phantom: PhantomData,
		}
	}

	/// Set cache capacity
	pub fn with_cache(self, capacity: usize) -> Self {
		Self {
			pattern: self.pattern.with_cache_strategy(
				crate::CacheStrategy::new(capacity)
			),
			..self
		}
	}

	/// Set subscription configuration
	pub fn with_config(self, config: SubscriptionConfig) -> Self {
		Self { config, ..self }
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
		let validated_pattern = self.pattern.with_compatible_pattern(new_pattern)?;

		Ok(Self {
			pattern: validated_pattern,
			..self
		})
	}

	/// Subscribe using configured parameters
	pub async fn subscribe<F, PayloadType>(
		self,
		client: &MqttClient<F>,
	) -> Result<MqttTopicSubscriber<MessageType, PayloadType, F>, MqttClientError>
	where
		MessageType: FromMqttMessage<PayloadType, F::DeserializeError>,
		PayloadType: Send + Sync + 'static,
		F: Default + Clone + Send + Sync + MessageSerializer<PayloadType>,
	{
		let subscriber = client
			.subscribe_with_config(self.pattern, self.config)
			.await?;
		Ok(MqttTopicSubscriber::new(subscriber))
	}
}
