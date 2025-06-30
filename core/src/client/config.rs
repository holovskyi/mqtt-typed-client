//! Configuration for MQTT client initialization

use std::marker::PhantomData;

use rumqttc::{MqttOptions, OptionError, LastWill};

use crate::{ MessageSerializer, MqttClientError, TypedLastWill};

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
    /// # use mqtt_typed_client_core::{MqttClientConfig, BincodeSerializer};
    /// # use mqtt_typed_client_macros::mqtt_topic;
    /// # #[derive(Debug)]
    /// # #[mqtt_topic("devices/{device_id}/status")]
    /// # struct DeviceStatus { device_id: u32, payload: String }
    /// let mut config = MqttClientConfig::<BincodeSerializer>::new("client", "broker", 1883);
    /// let last_will = DeviceStatus::last_will(123, "offline".to_string());
    /// config.with_last_will(last_will)?;
    /// # Ok::<(), mqtt_typed_client_core::MqttClientError>(())
    /// ```
    ///
    /// # Errors
    /// Returns `MqttClientError::Serialization` if payload serialization fails.
    pub fn with_last_will<T>(&mut self, last_will: TypedLastWill<T>) -> Result<&mut Self, MqttClientError>
    where 
        S: MessageSerializer<T>
    {
        let serializer = S::default();
        let payload = serializer.serialize(&last_will.payload)
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
