# MQTT Typed Client

A Rust library providing a typed MQTT client with pattern-based routing and automatic subscription management.

[![Crates.io](https://img.shields.io/crates/v/mqtt_typed_client.svg)](https://crates.io/crates/mqtt_typed_client)
[![Documentation](https://docs.rs/mqtt_typed_client/badge.svg)](https://docs.rs/mqtt_typed_client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

## Features

- 🔐 **Type-safe Message Handling**: Compile-time guarantees for message serialization/deserialization
- 🔍 **Pattern-based Routing**: Full support for MQTT wildcard patterns (`+`, `#`)
- 🚀 **Automatic Subscription Management**: Handles subscription lifecycle automatically
- 🛑 **Graceful Shutdown**: Proper resource cleanup and connection termination
- ⚡ **Async/Await Support**: Built on top of `tokio` for high-performance async operations
- 🔄 **Error Handling**: Comprehensive error types with automatic retry logic
- 📦 **Pluggable Serialization**: Bincode serializer included, easy to add custom serializers
- 🏃‍♂️ **Production Ready**: Memory-efficient with proper backpressure handling

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
mqtt_typed_client = "0.1"
serde = { version = "1.0", features = ["derive"] }
bincode = "2.0"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Usage

```rust
use mqtt_typed_client::prelude::*;
use serde::{Deserialize, Serialize};
use bincode::{Encode, Decode};

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
struct SensorData {
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create MQTT client
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(
        "mqtt://broker.hivemq.com:1883"
    ).await?;

    // Create a typed publisher
    let publisher = client.get_publisher::<SensorData>("sensors/temperature")?;

    // Create a typed subscriber with wildcard pattern
    let mut subscriber = client.subscribe::<SensorData>("sensors/+").await?;

    // Publish data
    let data = SensorData {
        temperature: 23.5,
        humidity: 45.0,
        timestamp: 1234567890,
    };
    publisher.publish(&data).await?;

    // Receive data
    if let Some((topic, result)) = subscriber.receive().await {
        match result {
            Ok(sensor_data) => {
                println!("Received from {}: {:?}", topic, sensor_data);
            }
            Err(e) => eprintln!("Deserialization error: {:?}", e),
        }
    }

    // Graceful shutdown
    connection.shutdown().await?;
    Ok(())
}
```

## Pattern Matching

The library supports MQTT topic pattern matching with wildcards:

### Single-level Wildcard (`+`)

Matches exactly one topic level:

```rust
// Subscribe to temperature from any room
let subscriber = client.subscribe::<f64>("home/+/temperature").await?;

// This will match:
// home/kitchen/temperature
// home/bedroom/temperature
// home/livingroom/temperature

// This will NOT match:
// home/kitchen/sensor/temperature (too many levels)
// office/kitchen/temperature (wrong prefix)
```

### Multi-level Wildcard (`#`)

Matches any number of topic levels (must be last):

```rust
// Subscribe to all sensors data
let subscriber = client.subscribe::<SensorData>("sensors/#").await?;

// This will match:
// sensors/temperature
// sensors/kitchen/temperature
// sensors/outdoor/weather/humidity
// sensors/anything/nested/deeply
```

### Combined Patterns

```rust
// Subscribe to any device status in any room
let subscriber = client.subscribe::<DeviceStatus>("+/devices/+/status").await?;

// Subscribe to all data from living room
let subscriber = client.subscribe::<serde_json::Value>("home/livingroom/#").await?;
```

## Custom Serialization

Implement the `MessageSerializer` trait for custom serialization:

### JSON Serializer Example

```rust
use mqtt_typed_client::MessageSerializer;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Default)]
pub struct JsonSerializer;

impl<T> MessageSerializer<T> for JsonSerializer
where
    T: Serialize + DeserializeOwned + 'static,
{
    type SerializeError = serde_json::Error;
    type DeserializeError = serde_json::Error;

    fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
        serde_json::to_vec(data)
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
        serde_json::from_slice(bytes)
    }
}

// Use with your client
let (client, connection) = MqttClient::<JsonSerializer>::connect("mqtt://broker.example.com").await?;
```

### MessagePack Serializer Example

```rust
use mqtt_typed_client::MessageSerializer;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Default)]
pub struct MessagePackSerializer;

impl<T> MessageSerializer<T> for MessagePackSerializer
where
    T: Serialize + DeserializeOwned + 'static,
{
    type SerializeError = rmp_serde::encode::Error;
    type DeserializeError = rmp_serde::decode::Error;

    fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
        rmp_serde::to_vec(data)
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
        rmp_serde::from_slice(bytes)
    }
}
```

## Advanced Configuration

### Publisher Configuration

```rust
let publisher = client
    .get_publisher::<SensorData>("sensors/temperature")?
    .with_qos(QoS::ExactlyOnce)  // Set QoS level
    .with_retain(true);          // Set retain flag

publisher.publish(&data).await?;
```

### Subscription Configuration

```rust
use mqtt_typed_client::routing::SubscriptionConfig;

let config = SubscriptionConfig {
    qos: QoS::ExactlyOnce,
};

let subscriber = client
    .subscribe_with_config::<SensorData>("sensors/+", config)
    .await?;
```

### Connection Options

```rust
// With authentication
let (client, connection) = MqttClient::<BincodeSerializer>::connect(
    "mqtt://username:password@broker.example.com:1883"
).await?;

// With TLS
let (client, connection) = MqttClient::<BincodeSerializer>::connect(
    "mqtts://broker.example.com:8883"
).await?;

// With client ID
let (client, connection) = MqttClient::<BincodeSerializer>::connect(
    "mqtt://broker.example.com:1883?client_id=my-unique-client"
).await?;
```

## Error Handling

The library provides comprehensive error handling:

```rust
use mqtt_typed_client::{MqttClientError, TopicRouterError};

match client.subscribe::<SensorData>("invalid/pattern/+/+").await {
    Ok(subscriber) => {
        // Handle successful subscription
    }
    Err(MqttClientError::TopicPattern(e)) => {
        eprintln!("Invalid topic pattern: {}", e);
    }
    Err(MqttClientError::Connection(e)) => {
        eprintln!("Connection failed: {}", e);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Performance Considerations

### Memory Usage

- The library uses `Arc<T>` for sharing messages between multiple subscribers
- Subscription channels have a default capacity of 500 messages
- Automatic cleanup of inactive subscriptions

### Backpressure Handling

```rust
// Slow subscribers are handled gracefully
// Messages to slow subscribers get a 2-second timeout
// After timeout, the message is dropped but subscription remains active
```

### Connection Resilience

- Automatic reconnection with exponential backoff
- Configurable retry limits and timeouts
- Graceful handling of network interruptions

## Wildcard Limitations

### Publisher Restrictions

Multi-level wildcards (`#`) **cannot be used with publishers** because they represent 
variable-length topic segments that cannot be constructed from fixed parameters.

```rust
// ❌ This will cause a compile error
#[mqtt_topic("events/{category}/#")]
struct Event { category: String, payload: String }

// ✅ Solution 1: Explicit subscriber mode
#[mqtt_topic("events/{category}/#", subscriber)]
struct EventReceived { category: String, payload: String }

// ✅ Solution 2: Separate structs for different purposes
#[mqtt_topic("events/{category}", publisher)]
struct EventToPublish { category: String, payload: String }

#[mqtt_topic("events/{category}/#", subscriber)]  
struct EventReceived { category: String, payload: String }
```

### Why This Limitation Exists

```rust
// How would the publisher generate a topic from this?
#[mqtt_topic("alerts/{severity}/{details:#}")]
//                              ^^^^^^^^^^^^
//                              Could be: "building1/floor2"
//                              Or:       "building1/floor2/room5/sensor3"
//                              Or:       "any/number/of/segments"
```

Publishers need to construct **concrete topic strings**, while `#` represents 
**variable-length paths** that cannot be determined at compile time.

### Helpful Error Messages

The macro provides clear guidance when you encounter this limitation:

```rust
#[mqtt_topic("alerts/{severity}/{details:#}")]
struct Alert { severity: String, details: String, payload: String }
```

```
error: Cannot generate publisher methods for patterns with '#' wildcards.

 Solutions:
   • Use subscriber-only mode: #[mqtt_topic("alerts/{severity}/{details:#}", subscriber)]
   • Create separate structs for different purposes:

     #[mqtt_topic("alerts/{severity}/{details:#}", subscriber)]
     struct AlertReceived { /* fields */ }

     #[mqtt_topic("alerts/{severity}", publisher)]
     struct AlertToSend { /* fields */ }

 Why: Publishers need concrete topic strings, but '#' represents 
 variable-length paths that cannot be determined at compile time.
```

## Examples

Check out the `examples/` directory for more comprehensive examples:

- `basic_usage.rs` - Simple publish/subscribe example
- `patterns.rs` - Wildcard pattern matching examples
- `custom_serializer.rs` - Custom serialization implementation
- `error_handling.rs` - Comprehensive error handling

Run examples with:

```bash
cargo run --example main_example.rs 
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
