<div align="center">

# 🦀 MQTT Typed Client

A **high-level, asynchronous, type-safe MQTT client** built on top of rumqttc

[![Crates.io](https://img.shields.io/crates/v/mqtt-typed-client.svg)](https://crates.io/crates/mqtt-typed-client)
[![Documentation](https://docs.rs/mqtt-typed-client/badge.svg)](https://docs.rs/mqtt-typed-client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.85.1-blue.svg)](https://blog.rust-lang.org/2025/01/09/Rust-1.85.0.html)

**Zero-cost abstractions** and **compile-time guarantees** for MQTT communication

</div>

## ✨ Key Features

- **Type-safe topic patterns** with named parameters and automatic parsing
- **Zero-cost abstractions** via procedural macros with compile-time validation
- **Built-in serialization** support for 8+ formats (Bincode, JSON, MessagePack, etc.)
- **Efficient message routing** with tree-based topic matching and internal caching
- **Smart defaults** with full configurability when needed
- **Memory efficient** design with proper resource management
- **Automatic reconnection** and graceful shutdown

⚠️ **MSRV**: Rust 1.85.1 (driven by default `bincode` serializer; can be lowered with alternative serializers)

## 🚀 Quick Start

Add to your `Cargo.toml`:
```toml
[dependencies]
mqtt-typed-client = "0.1"
```

### Recommended: Type-Safe Approach with Macros

```rust
use mqtt_typed_client::prelude::*;
use mqtt_typed_client_macros::mqtt_topic;
use serde::{Deserialize, Serialize};
use bincode::{Encode, Decode};

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
struct SensorReading {
    temperature: f64,
    humidity: f64,
    timestamp: u64,
}

// Define typed topic with automatic parameter extraction
#[mqtt_topic("sensors/{location}/{sensor_type}/data")]
struct SensorTopic {
    location: String,
    sensor_type: String,
    payload: SensorReading,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to MQTT broker
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(
        "mqtt://broker.hivemq.com:1883"
    ).await?;

    // Type-safe operations with automatic topic parameter handling
    let topic_client = client.sensor_topic();
    
    // Subscribe to all sensors: "sensors/+/+/data"
    let mut subscriber = topic_client.subscribe().await?;
    
    // Publish to specific topic: "sensors/kitchen/temperature/data"
    let reading = SensorReading { 
        temperature: 22.5, 
        humidity: 45.0, 
        timestamp: 1234567890 
    };
    topic_client.publish("kitchen", "temperature", &reading).await?;
    
    // Receive with automatic parameter extraction
    if let Some(Ok(msg)) = subscriber.receive().await {
        println!("Sensor {} in {} reported: temp={}°C, humidity={}%", 
            msg.sensor_type, msg.location, 
            msg.payload.temperature, msg.payload.humidity);
    }
    
    connection.shutdown().await?;
    Ok(())
}
```

## 🆚 What mqtt-typed-client adds over rumqttc

**Publishing:**
```rust
// rumqttc - manual topic construction and serialization
let topic = format!("sensors/{}/temperature", sensor_id);
let payload = serde_json::to_vec(&data)?;
client.publish(topic, QoS::AtLeastOnce, false, payload).await?;

// mqtt-typed-client - type-safe, automatic
topic_client.publish(&sensor_id, &data).await?;
```

**Subscribing with routing:**
```rust
// rumqttc - manual pattern matching and dispatching
// while let Ok(event) = eventloop.poll().await {
//     if let Event::Incoming(Packet::Publish(publish)) = event {
//         if publish.topic.starts_with("sensors/") {
//             // Manual topic parsing, manual deserialization...
//         } else if publish.topic.starts_with("alerts/") {
//             // More manual parsing...
//         }
//     }
// }

// mqtt-typed-client - automatic routing to typed handlers
let mut sensor_sub = client.sensor_topic().subscribe().await?;
let mut alert_sub = client.alert_topic().subscribe().await?;

tokio::select! {
    msg = sensor_sub.receive() => { /* typed sensor data ready */ }
    msg = alert_sub.receive() => { /* typed alert data ready */ }
}
```

📋 **For detailed comparison see:** [docs/COMPARISON_WITH_RUMQTTC.md](docs/COMPARISON_WITH_RUMQTTC.md)

## 📦 Serialization Support

Multiple serialization formats are supported via feature flags:

- `bincode` - Binary serialization (default, most efficient)
- `json` - JSON serialization (default, human-readable)
- `messagepack` - MessagePack binary format
- `cbor` - CBOR binary format
- `postcard` - Embedded-friendly binary format
- `ron` - Rusty Object Notation
- `flexbuffers` - FlatBuffers FlexBuffers
- `protobuf` - Protocol Buffers (requires generated types)

Enable additional serializers:
```toml
[dependencies]
mqtt-typed-client = { version = "0.1", features = ["messagepack", "cbor"] }
```

Custom serializers can be implemented by implementing the `MessageSerializer` trait.

## 🎯 Topic Pattern Matching

Supports MQTT wildcard patterns with named parameters:

- `{param}` - Named parameter (equivalent to `+` wildcard)
- `{param:#}` - Multi-level named parameter (equivalent to `#` wildcard)

```rust
// Traditional MQTT wildcards
#[mqtt_topic("home/+/temperature")]     // matches: home/kitchen/temperature
struct SimplePattern { payload: f64 }

// Named parameters (recommended)
#[mqtt_topic("home/{room}/temperature")] // matches: home/kitchen/temperature  
struct NamedPattern { 
    room: String,        // Automatically extracted: "kitchen"
    payload: f64 
}

// Multi-level parameters
#[mqtt_topic("logs/{service}/{path:#}")]  // matches: logs/api/v1/users/create
struct LogPattern {
    service: String,     // "api"
    path: String,        // "v1/users/create"  
    payload: Data
}
```

## 📚 Examples

See [`crate::examples`] - Complete usage examples with source code

- `000_hello_world.rs` - Basic publish/subscribe with macros
- `001_ping_pong.rs` - Multi-client communication
- `002_configuration.rs` - Advanced client configuration
- `003_hello_world_lwt.rs` - Last Will & Testament
- `004_hello_world_tls.rs` - TLS/SSL connections
- `005_hello_world_serializers.rs` - Custom serializers
- `006_retain_and_clear.rs` - Retained messages
- `007_custom_patterns.rs` - Custom topic patterns
- `008_modular_example.rs` - Modular application structure

Run examples:
```bash
cargo run --example 000_hello_world
```

## 🔧 Advanced Usage: Low-Level API

For cases where you need direct control without macros:

```rust
use mqtt_typed_client::prelude::*;
use serde::{Deserialize, Serialize};
use bincode::{Encode, Decode};

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
struct SensorData {
    temperature: f64,
    humidity: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(
        "mqtt://broker.hivemq.com:1883"
    ).await?;

    // Direct topic operations
    let publisher = client.get_publisher::<SensorData>("sensors/temperature")?;
    let mut subscriber = client.subscribe::<SensorData>("sensors/+").await?;

    let data = SensorData { temperature: 23.5, humidity: 45.0 };
    publisher.publish(&data).await?;

    if let Some((topic, result)) = subscriber.receive().await {
        match result {
            Ok(sensor_data) => println!("Received from {}: {:?}", topic.topic_path(), sensor_data),
            Err(e) => eprintln!("Deserialization error: {:?}", e),
        }
    }

    connection.shutdown().await?;
    Ok(())
}
```

## 📄 License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.
