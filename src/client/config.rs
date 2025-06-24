//! Configuration for MQTT client initialization

/// Configuration parameters for MQTT client creation
#[derive(Debug, Clone)]
pub struct MqttClientConfig {
    /// Size of the topic path cache (must be > 0)
    pub topic_cache_size: usize,
    /// Capacity of the event loop channel
    pub event_loop_capacity: usize,
    /// Capacity of the command channel for subscription manager
    pub command_channel_capacity: usize,
    /// Capacity of the unsubscribe channel
    pub unsubscribe_channel_capacity: usize,
}

impl Default for MqttClientConfig {
    fn default() -> Self {
        Self {
            topic_cache_size: 100,
            event_loop_capacity: 10,
            command_channel_capacity: 100,
            unsubscribe_channel_capacity: 10,
        }
    }
}
