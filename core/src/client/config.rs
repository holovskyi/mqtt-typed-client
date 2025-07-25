//! Configuration for MQTT client initialization

use std::marker::PhantomData;

use rumqttc::{LastWill, MqttOptions, OptionError};

use crate::{MessageSerializer, MqttClientError, TypedLastWill};

/// Client-level performance and behavior settings for the MQTT typed client.
///
/// These settings control internal resource allocation, performance characteristics,
/// and operational behavior of the MQTT client. Proper tuning can significantly
/// impact memory usage, throughput, and responsiveness under different workloads.
///
/// # Performance Tuning Guide
///
/// ## Low-Resource Environments (IoT devices, embedded systems):
/// ```rust
/// use mqtt_typed_client_core::ClientSettings;
/// 
/// ClientSettings {
///     topic_cache_size: 20,           // Minimal cache for memory conservation
///     event_loop_capacity: 5,         // Small buffer for low throughput
///     command_channel_capacity: 20,   // Reduced command queue
///     unsubscribe_channel_capacity: 5, // Minimal unsubscribe queue
///     connection_timeout_millis: 10000, // Longer timeout for slower networks
/// };
/// ```
///
/// ## High-Throughput Applications (real-time data processing):
/// ```rust
/// use mqtt_typed_client_core::ClientSettings;
/// 
/// ClientSettings {
///     topic_cache_size: 1000,         // Large cache for performance
///     event_loop_capacity: 100,       // Large buffer for message bursts
///     command_channel_capacity: 500,  // High command throughput
///     unsubscribe_channel_capacity: 50, // Handle dynamic subscriptions
///     connection_timeout_millis: 3000, // Fast timeout for responsive systems
/// };
/// ```
///
/// ## Balanced General Use (recommended defaults):
/// ```rust
/// use mqtt_typed_client_core::ClientSettings;
/// 
/// ClientSettings::default(); // Uses conservative but performant settings
/// ```
#[derive(Debug, Clone)]
pub struct ClientSettings {
	/// **Topic Path Cache Size** - LRU cache for parsed topic structures.
	///
	/// Caches `TopicPath` objects created from incoming MQTT topic strings to avoid
	/// repeated string parsing overhead. Each cache entry stores ~50-150 bytes.
	///
	/// **Impact:**
	/// - **Memory**: ~50-150 bytes × cache_size
	/// - **CPU**: Reduces topic string parsing for repeated topics
	/// - **Latency**: Faster message routing for cached topics
	///
	/// **Tuning Guidelines:**
	/// - **Low traffic** (< 100 unique topics): 50-100
	/// - **Medium traffic** (100-1000 unique topics): 200-500  
	/// - **High traffic** (1000+ unique topics): 500-2000
	/// - **Memory constrained**: 10-50
	///
	/// **Note:** Must be > 0. Cache uses LRU eviction policy.
	///
	/// **Default:** 100 (suitable for most applications)
	pub topic_cache_size: usize,

	/// **Event Loop Channel Capacity** - Internal buffer for MQTT protocol messages.
	///
	/// Controls the buffer size of the underlying rumqttc event loop channel.
	/// This affects how many MQTT protocol packets (ConnAck, Publish, PubAck, etc.)
	/// can be queued internally before backpressure occurs.
	///
	/// **Impact:**
	/// - **Memory**: ~100-500 bytes × capacity (depends on message sizes)
	/// - **Throughput**: Higher capacity = better burst handling
	/// - **Latency**: Lower capacity = more responsive to backpressure
	/// - **Reliability**: Buffer overflow causes connection instability
	///
	/// **Tuning Guidelines:**
	/// - **Low throughput** (< 10 msgs/sec): 5-20
	/// - **Medium throughput** (10-100 msgs/sec): 20-50
	/// - **High throughput** (100+ msgs/sec): 50-200
	/// - **Burst workloads**: 100-500
	///
	/// **Symptoms of undersized buffer:**
	/// - Connection drops under load
	/// - Message delivery delays
	/// - "Channel full" errors in logs
	///
	/// **Default:** 10 (conservative, suitable for moderate loads)
	pub event_loop_capacity: usize,

	/// **Command Channel Capacity** - Queue size for subscription management commands.
	///
	/// Controls the buffer for internal commands between the client API and the
	/// subscription manager actor. Commands include: subscribe, send, resubscribe_all.
	/// Each command represents a method call like `client.subscribe()` or message dispatch.
	///
	/// **Impact:**
	/// - **Memory**: ~200-1000 bytes × capacity (varies by command type)
	/// - **API responsiveness**: Higher capacity = less blocking on rapid API calls
	/// - **Concurrency**: Enables better parallelism between API and message processing
	///
	/// **Tuning Guidelines:**
	/// - **Sequential usage**: 10-50 (one command at a time)
	/// - **Concurrent subscriptions**: 50-200 (multiple subscribe() calls)
	/// - **High-frequency operations**: 200-1000 (frequent sub/unsub)
	/// - **Batch operations**: 500+ (bulk subscription changes)
	///
	/// **Symptoms of undersized buffer:**
	/// - API calls block/timeout
	/// - Slow subscription setup
	/// - "Command channel full" errors
	///
	/// **Default:** 100 (handles moderate concurrent usage)
	pub command_channel_capacity: usize,

	/// **Unsubscribe Channel Capacity** - Queue size for cleanup operations.
	///
	/// Controls the buffer for unsubscribe notifications from dropped subscribers.
	/// When a subscriber is dropped (goes out of scope), it sends an unsubscribe
	/// notification through this channel for cleanup.
	///
	/// **Impact:**
	/// - **Memory**: ~50-100 bytes × capacity
	/// - **Cleanup latency**: Higher capacity = delayed cleanup under burst drops
	/// - **Resource leaks**: Undersized buffer can delay topic cleanup
	///
	/// **Tuning Guidelines:**
	/// - **Stable subscriptions**: 5-20 (subscribers rarely drop)
	/// - **Dynamic subscriptions**: 20-100 (frequent sub/unsub cycles)
	/// - **High churn rate**: 100+ (many short-lived subscribers)
	///
	/// **Symptoms of undersized buffer:**
	/// - Memory leaks (uncleaned topics)
	/// - Broker still sending to unsubscribed topics
	/// - "Unsubscribe channel full" warnings
	///
	/// **Default:** 10 (sufficient for typical cleanup patterns)
	pub unsubscribe_channel_capacity: usize,

	/// **Connection Timeout** - Maximum time to wait for initial MQTT connection.
	///
	/// Controls how long `MqttClient::connect()` will wait for the broker to
	/// respond with a successful ConnAck packet before timing out.
	///
	/// **Impact:**
	/// - **Startup time**: Longer timeout = slower failure detection
	/// - **Network resilience**: Shorter timeout = less tolerance for slow networks
	/// - **User experience**: Affects perceived application responsiveness
	///
	/// **Tuning Guidelines:**
	/// - **Local broker** (localhost): 1000-3000ms
	/// - **LAN broker** (same network): 3000-5000ms  
	/// - **Internet broker** (remote): 5000-15000ms
	/// - **Unreliable networks**: 10000-30000ms
	/// - **Fast-fail applications**: 1000-3000ms
	///
	/// **Trade-offs:**
	/// - **Shorter timeout**: Faster error feedback, may fail on slow networks
	/// - **Longer timeout**: More network tolerance, slower startup failures
	///
	/// **Default:** 5000ms (5 seconds, balanced for most network conditions)
	pub connection_timeout_millis: u64,
}

impl Default for ClientSettings {
	fn default() -> Self {
		Self {
			topic_cache_size: 100,
			event_loop_capacity: 10,
			command_channel_capacity: 100,
			unsubscribe_channel_capacity: 10,
			connection_timeout_millis: 5000,
		}
	}
}

/// Configuration for MQTT client creation
#[derive(Debug, Clone)]
pub struct MqttClientConfig<S> {
	/// Underlying MQTT connection options (from rumqttc)
	pub connection: MqttOptions,
	/// Client-level performance and behavior settings
	pub settings: ClientSettings,
	/// Phantom data for serializer type
	_serializer: PhantomData<S>,
}

impl<S> MqttClientConfig<S> {
	/// Create config with default settings
	pub fn new(client_id: &str, host: &str, port: u16) -> Self {
		Self {
			connection: MqttOptions::new(client_id, host, port),
			settings: ClientSettings::default(),
			_serializer: PhantomData,
		}
	}

	/// Parse configuration from MQTT URL
	///
	/// Supports: tcp://, mqtt://, ssl://, mqtts://, ws://, wss://
	pub fn from_url(url: &str) -> Result<Self, OptionError> {
		Ok(Self {
			connection: MqttOptions::parse_url(url)?,
			settings: ClientSettings::default(),
			_serializer: PhantomData,
		})
	}

	/// Create config for localhost:1883
	pub fn localhost(client_id: &str) -> Self {
		Self::new(client_id, "localhost", 1883)
	}

	/// Configure Last Will and Testament message
	///
	/// The Last Will message will be published by the broker if this client
	/// disconnects unexpectedly. Payload is serialized immediately using the
	/// configured serializer type.
	///
	/// # Example
	/// ```rust
	/// # use mqtt_typed_client_core::{MqttClientConfig, BincodeSerializer, TypedLastWill};
	/// # use rumqttc::QoS;
	/// let mut config = MqttClientConfig::<BincodeSerializer>::new("client", "broker", 1883);
	///
	/// // Create last will manually (in real code, use generated methods from #[mqtt_topic])
	/// let last_will = TypedLastWill {
	///     topic: "devices/123/status".to_string(),
	///     payload: "offline".to_string(),
	///     qos: QoS::AtLeastOnce,
	///     retain: true,
	/// };
	///
	/// config.with_last_will(last_will)?;
	/// # Ok::<(), mqtt_typed_client_core::MqttClientError>(())
	/// ```
	///
	/// # Errors
	/// Returns `MqttClientError::Serialization` if payload serialization fails.
	pub fn with_last_will<T>(
		&mut self,
		last_will: TypedLastWill<T>,
	) -> Result<&mut Self, MqttClientError>
	where
		S: MessageSerializer<T>,
	{
		let serializer = S::default();
		let payload = serializer
			.serialize(&last_will.payload)
			.map_err(|e| MqttClientError::Serialization(format!("{e:?}")))?;

		self.connection.set_last_will(LastWill::new(
			last_will.topic,
			payload,
			last_will.qos,
			last_will.retain,
		));

		Ok(self)
	}
}
