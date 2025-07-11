use std::marker::PhantomData;

use arcstr::ArcStr;

use crate::{
	MessageSerializer, MqttClient, MqttClientError, MqttTopicSubscriber,
	SubscriptionConfig, TopicPatternError, TopicPatternPath,
	structured::FromMqttMessage,
};

/// Immutable builder for configuring MQTT subscriptions
#[derive(Debug)]
pub struct SubscriptionBuilder<MessageType, F: Clone> {
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
where F: Clone
{
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

	/// Enable LRU cache for topic parsing optimization.
	///
	/// When the same MQTT topic paths are received repeatedly, caching the parsing results
	/// can significantly improve performance by avoiding redundant topic pattern matching.
	///
	/// # Parameters
	/// * `capacity` - Maximum number of topic parsing results to cache (LRU eviction)
	///   Set to 0 to disable caching entirely
	///
	/// # Performance Impact
	/// - **Memory**: ~50-200 bytes per cached topic (depending on topic complexity)
	/// - **CPU**: Reduces topic parsing overhead for repeated patterns
	/// - **Use case**: Most beneficial when same topics repeat frequently
	///
	/// # Examples
	/// ```rust,ignore
	/// // Cache last 100 topic parsing results
	/// let subscriber = client.my_topic()
	///     .subscription()
	///     .with_cache(100)
	///     .subscribe().await?;
	///
	/// // Disable caching for memory-constrained environments
	/// let subscriber = client.my_topic()
	///     .subscription()
	///     .with_cache(0)
	///     .subscribe().await?;
	/// ```
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

	/// Override the default topic pattern with a custom one.
	///
	/// This allows using a different MQTT topic pattern than the one defined in the
	/// `#[mqtt_topic]` macro, while ensuring the parameter structure remains compatible.
	/// The new pattern must have the same parameter names and types as the original.
	///
	/// # Parameters
	/// * `custom_pattern` - New topic pattern string (e.g., "sensors/{location}/data/{sensor_id}")
	///
	/// # Compatibility Requirements
	/// - Same parameter names: `{location}`, `{sensor_id}`, etc.
	/// - Same parameter order and count
	/// - Parameter types must match the struct fields
	///
	/// # Use Cases
	/// - **Environment-specific patterns**: Different topic structures for dev/prod
	/// - **Multi-tenant systems**: Adding tenant prefixes to topics
	/// - **Legacy compatibility**: Supporting old topic formats
	/// - **A/B testing**: Different topic patterns for the same data structure
	///
	/// # Examples
	/// ```rust,ignore
	/// // Original pattern from macro: "sensors/{location}/{sensor_id}"
	/// // Override with environment-specific pattern:
	/// let subscriber = client.sensor_topic()
	///     .subscription()
	///     .with_pattern("prod/sensors/{location}/device/{sensor_id}")?  // ✅ Compatible
	///     .subscribe().await?;
	///
	/// // Multi-tenant pattern:
	/// let subscriber = client.sensor_topic()
	///     .subscription()
	///     .with_pattern("tenant_123/sensors/{location}/{sensor_id}")?  // ✅ Compatible
	///     .subscribe().await?;
	///
	/// // ❌ This would fail - different parameter names:
	/// // .with_pattern("sensors/{room}/{device_id}")  // Error: parameter mismatch
	/// ```
	///
	/// # Errors
	/// Returns `MqttClientError` if:
	/// - Pattern syntax is invalid
	/// - Parameter names don't match the original pattern
	/// - Parameter count differs from the original pattern
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
