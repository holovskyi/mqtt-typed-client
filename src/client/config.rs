//! Configuration for MQTT client initialization

use rumqttc::{MqttOptions, OptionError};

/// Client-level performance and behavior settings
#[derive(Debug, Clone)]
pub struct ClientSettings {
    /// Size of the topic path cache (must be > 0)
    pub topic_cache_size: usize,
    /// Capacity of the event loop channel
    pub event_loop_capacity: usize,
    /// Capacity of the command channel for subscription manager
    pub command_channel_capacity: usize,
    /// Capacity of the unsubscribe channel
    pub unsubscribe_channel_capacity: usize,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            topic_cache_size: 100,
            event_loop_capacity: 10,
            command_channel_capacity: 100,
            unsubscribe_channel_capacity: 10,
        }
    }
}

/// Configuration for MQTT client creation
#[derive(Debug, Clone)]
pub struct MqttClientConfig {
    /// Underlying MQTT connection options (from rumqttc)
    pub connection: MqttOptions,
    /// Client-level performance and behavior settings
    pub settings: ClientSettings,
}

impl MqttClientConfig {
    /// Create new config with common defaults
    ///
    /// # Arguments
    /// * `client_id` - Unique identifier for this MQTT client
    /// * `host` - MQTT broker hostname or IP address
    /// * `port` - MQTT broker port number
    ///
    /// # Example
    /// ```rust
    /// use mqtt_typed_client::MqttClientConfig;
    /// 
    /// let config = MqttClientConfig::new("my_client", "broker.hivemq.com", 1883);
    /// ```
    pub fn new(client_id: &str, host: &str, port: u16) -> Self {
        Self {
            connection: MqttOptions::new(client_id, host, port),
            settings: ClientSettings::default(),
        }
    }
    
    /// Parse configuration from URL string
    ///
    /// Supports URLs with protocols: tcp://, mqtt://, ssl://, mqtts://, ws://, wss://
    ///
    /// # Arguments
    /// * `url` - MQTT broker URL (e.g., "mqtt://broker.hivemq.com:1883?client_id=my_client")
    ///
    /// # Example
    /// ```rust
    /// use mqtt_typed_client::MqttClientConfig;
    /// 
    /// let config = MqttClientConfig::from_url("mqtt://broker.hivemq.com:1883?client_id=my_client")?;
    /// # Ok::<(), rumqttc::OptionError>(())
    /// ```
    pub fn from_url(url: &str) -> Result<Self, OptionError> {
        Ok(Self {
            connection: MqttOptions::parse_url(url)?,
            settings: ClientSettings::default(),
        })
    }
    
    /// Convenience method for localhost development
    ///
    /// Creates a config connecting to localhost:1883 with the specified client ID.
    ///
    /// # Arguments
    /// * `client_id` - Unique identifier for this MQTT client
    ///
    /// # Example
    /// ```rust
    /// use mqtt_typed_client::MqttClientConfig;
    /// 
    /// let config = MqttClientConfig::localhost("test_client");
    /// ```
    pub fn localhost(client_id: &str) -> Self {
        Self::new(client_id, "localhost", 1883)
    }
}
