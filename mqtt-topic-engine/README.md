<div align="center">

# MQTT Topic Engine

**High-performance MQTT topic pattern matching and routing engine for Rust**

[![Crates.io](https://img.shields.io/crates/v/mqtt-topic-engine.svg)](https://crates.io/crates/mqtt-topic-engine)
[![Documentation](https://docs.rs/mqtt-topic-engine/badge.svg)](https://docs.rs/mqtt-topic-engine)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

Zero-dependency (by default) topic pattern parser, matcher, and router for MQTT applications

</div>

## Features

- **Pattern Parsing** - Parse MQTT topic patterns with support for wildcards (`+`, `#`) and named parameters
- **Fast Matching** - Efficient tree-based topic matching algorithm with O(n) complexity
- **Optional Caching** - LRU cache for topic match results to boost performance (optional `lru-cache` feature)
- **Message Routing** - Route incoming messages to multiple subscribers based on topic patterns (optional `router` feature)
- **Named Parameters** - Extract topic segments as named parameters (`{sensor_id}`, `{path:#}`)
- **Modular Design** - Enable only the features you need for minimal binary size
- **Well Tested** - Comprehensive test suite with 100+ tests covering edge cases
- **Lightweight** - Minimal dependencies, optimized for embedded and performance-critical applications
- **MQTT Client Agnostic** - Works with any MQTT client library (rumqttc, paho-mqtt, ntex-mqtt, etc.)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mqtt-topic-engine = "0.1.0"
```

### Feature Flags

The library is highly modular with optional features:

```toml
# Default: all features enabled
mqtt-topic-engine = "0.1.0"

# Minimal: only pattern matching and validation
mqtt-topic-engine = { version = "0.1.0", default-features = false }

# Pattern matching + routing (without LRU cache)
mqtt-topic-engine = { version = "0.1.0", default-features = false, features = ["router"] }

# Pattern matching + LRU cache (without routing)
mqtt-topic-engine = { version = "0.1.0", default-features = false, features = ["lru-cache"] }

# With specific MQTT client integration
mqtt-topic-engine = { version = "0.1.0", features = ["rumqttc"] }
mqtt-topic-engine = { version = "0.1.0", features = ["paho-mqtt"] }
mqtt-topic-engine = { version = "0.1.0", features = ["ntex-mqtt"] }
```

**Available features:**

| Feature | Description | Default |
|---------|-------------|----------|
| `router` | Topic routing with `TopicRouter` and `TopicMatcher` | ✅ |
| `lru-cache` | LRU caching for pattern matching results | ✅ |
| `rumqttc` | Integration with rumqttc MQTT client (QoS conversion) | ❌ |
| `paho-mqtt` | Integration with paho-mqtt client (QoS conversion) | ❌ |
| `ntex-mqtt` | Integration with ntex-mqtt client (QoS conversion) | ❌ |

## Quick Start

### Basic Pattern Matching

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Parse a topic pattern
let pattern = TopicPatternPath::new_from_string(
    "sensors/+/temperature",
    CacheStrategy::NoCache
)?;

// Check if a topic matches - just pass a string!
let topic = TopicPath::new("sensors/kitchen/temperature");
let topic_match = pattern.try_match(topic)?;

println!("Topic matched: {}", topic_match.topic_path());
```

### Named Parameters

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Pattern with named parameters
let pattern = TopicPatternPath::new_from_string(
    "devices/{location}/{device_id}/status",
    CacheStrategy::NoCache
)?;

let topic = TopicPath::new("devices/kitchen/sensor001/status");
let topic_match = pattern.try_match(topic)?;

// Extract parameters by name
let location = topic_match.get_named_param("location").unwrap();
let device_id = topic_match.get_named_param("device_id").unwrap();

println!("Location: {}, Device: {}", location, device_id);
// Output: Location: kitchen, Device: sensor001
```

### Multi-Level Wildcards

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Match all subtopics under logs/
let pattern = TopicPatternPath::new_from_string(
    "logs/{service}/{details:#}",
    CacheStrategy::NoCache
)?;

let topic = TopicPath::new("logs/api/v1/users/create");
let topic_match = pattern.try_match(topic)?;

let service = topic_match.get_named_param("service").unwrap();
let details = topic_match.get_named_param("details").unwrap();

println!("Service: {}, Path: {}", service, details);
// Output: Service: api, Path: v1/users/create
```

## Core Concepts

### Topic Patterns

MQTT topic engine supports standard MQTT wildcards plus named parameters:

| Pattern | Description | Example | Matches |
|---------|-------------|---------|---------|
| `sensors/temperature` | Exact match | `sensors/temperature` | `sensors/temperature` only |
| `sensors/+/data` | Single-level wildcard | `sensors/+/data` | `sensors/kitchen/data`, `sensors/bedroom/data` |
| `sensors/#` | Multi-level wildcard | `sensors/#` | `sensors`, `sensors/temp`, `sensors/kitchen/temp` |
| `sensors/{room}/temp` | Named parameter | `sensors/{room}/temp` | Same as `sensors/+/temp` but extracts `room` |
| `logs/{app}/{path:#}` | Named multi-level | `logs/{app}/{path:#}` | Same as `logs/+/#` but extracts both |

### Topic Router

Route messages to multiple subscribers based on patterns:

```rust
use mqtt_topic_engine::{TopicRouter, TopicPatternPath, TopicPath, CacheStrategy};
use rumqttc::QoS;

let mut router = TopicRouter::new();

// Add subscriptions
let pattern1 = TopicPatternPath::new_from_string("sensors/+/temperature", CacheStrategy::NoCache)?;
let (needs_subscribe, sub_id1) = router.add_subscription(
    pattern1,
    QoS::AtLeastOnce,
    "handler1"  // Your subscriber data (any type)
);

let pattern2 = TopicPatternPath::new_from_string("sensors/#", CacheStrategy::NoCache)?;
let (needs_subscribe, sub_id2) = router.add_subscription(
    pattern2,
    QoS::AtMostOnce,
    "handler2"
);

// Route incoming message
let topic = TopicPath::new("sensors/kitchen/temperature");
let subscribers = router.get_subscribers(&topic);

for (sub_id, (pattern, qos), handler) in subscribers {
    println!("Matched subscriber: {:?} with pattern: {}", sub_id, pattern.topic_pattern());
    // Forward message to handler
}
```

### Parameter Binding

Bind specific values to parameters for filtered subscriptions:

```rust
use mqtt_topic_engine::{TopicPatternPath, CacheStrategy};

let pattern = TopicPatternPath::new_from_string(
    "sensors/{location}/{sensor_type}/{sensor_id}/data",
    CacheStrategy::NoCache
)?;

// Only subscribe to temperature sensors in the kitchen
let filtered = pattern
    .bind_parameter("location", "kitchen")?
    .bind_parameter("sensor_type", "temperature")?;

println!("{}", filtered.mqtt_pattern());
// Output: sensors/kitchen/temperature/+/data
```

This is useful for:
- Dynamic subscription filtering
- Multi-tenant applications
- Selective message routing

## Advanced Examples

### LRU Caching for Performance

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};
use std::num::NonZeroUsize;

// Create pattern with LRU cache of 1000 entries
let cache_size = NonZeroUsize::new(1000).unwrap();
let pattern = TopicPatternPath::new_from_string(
    "sensors/{location}/{device}/data",
    CacheStrategy::Lru(cache_size)
)?;  

// First match - computed and cached
let topic1 = TopicPath::new("sensors/kitchen/temp001/data");
let match1 = pattern.try_match(topic1)?;

// Second match to same topic - retrieved from cache (faster!)
let topic2 = TopicPath::new("sensors/kitchen/temp001/data");
let match2 = pattern.try_match(topic2)?;
```

### Topic Pattern Validation

```rust
use mqtt_topic_engine::{TopicPatternPath, CacheStrategy, TopicPatternError};

// Valid patterns
assert!(TopicPatternPath::new_from_string("sensors/+/data", CacheStrategy::NoCache).is_ok());
assert!(TopicPatternPath::new_from_string("logs/#", CacheStrategy::NoCache).is_ok());
assert!(TopicPatternPath::new_from_string("home/{room}/temperature", CacheStrategy::NoCache).is_ok());

// Invalid patterns - # wildcard must be last
assert!(matches!(
    TopicPatternPath::new_from_string("sensors/#/invalid", CacheStrategy::NoCache),
    Err(TopicPatternError::HashPosition { .. })
));

// Empty pattern not allowed
assert!(matches!(
    TopicPatternPath::new_from_string("", CacheStrategy::NoCache),
    Err(TopicPatternError::EmptyTopic)
));
```

### Building Topics for Publishing

```rust
use mqtt_topic_engine::{TopicPatternPath, CacheStrategy};

let pattern = TopicPatternPath::new_from_string(
    "devices/{device_type}/{device_id}/command",
    CacheStrategy::NoCache
)?;

// Format topic with parameters
let topic = pattern.format_topic(&[&"sensor", &42])?;
println!("{}", topic);
// Output: devices/sensor/42/command

// Type-safe: compile error if wrong number of parameters
// let invalid = pattern.format_topic(&[&"sensor"])?; // Runtime error!
```

### Managing Subscriptions

```rust
use mqtt_topic_engine::{TopicRouter, TopicPatternPath, CacheStrategy};
use rumqttc::QoS;

let mut router = TopicRouter::new();

// Add subscription
let pattern = TopicPatternPath::new_from_string("sensors/+/data", CacheStrategy::NoCache)?;
let (needs_mqtt_subscribe, sub_id) = router.add_subscription(
    pattern,
    QoS::AtLeastOnce,
    "my_handler"
);

if needs_mqtt_subscribe {
    // Subscribe to MQTT broker
    println!("New subscription needed on broker");
}

// Later: remove subscription
let (needs_mqtt_unsubscribe, pattern) = router.unsubscribe(&sub_id)?;

if needs_mqtt_unsubscribe {
    // Unsubscribe from MQTT broker
    println!("Should unsubscribe from broker: {}", pattern.mqtt_pattern());
}
```

### Wildcard Patterns

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Anonymous wildcards (standard MQTT)
let pattern1 = TopicPatternPath::new_from_string("home/+/temperature", CacheStrategy::NoCache)?;

// Named wildcards (extracts parameter)
let pattern2 = TopicPatternPath::new_from_string("home/{room}/temperature", CacheStrategy::NoCache)?;

let topic = TopicPath::new("home/kitchen/temperature");

// Both match the same topics
let match1 = pattern1.try_match(topic.clone())?;
let match2 = pattern2.try_match(topic)?;

// But only named wildcards extract parameters
assert!(match2.get_named_param("room").is_some());
println!("Room: {}", match2.get_named_param("room").unwrap());
// Output: Room: kitchen
```

### Complex Routing Scenarios

```rust
use mqtt_topic_engine::{TopicRouter, TopicPatternPath, TopicPath, CacheStrategy};
use rumqttc::QoS;

let mut router = TopicRouter::<String>::new();

// Multiple overlapping patterns
let patterns = vec![
    ("home/kitchen/temperature", "exact_handler"),
    ("home/+/temperature", "any_room_temp_handler"),
    ("home/kitchen/+", "kitchen_all_sensors_handler"),
    ("home/#", "all_home_handler"),
    ("+/kitchen/#", "all_kitchen_subtopics_handler"),
];

for (pattern_str, handler_name) in patterns {
    let pattern = TopicPatternPath::new_from_string(pattern_str, CacheStrategy::NoCache)?;
    router.add_subscription(pattern, QoS::AtMostOnce, handler_name.to_string());
}

// Check which handlers receive the message
let topic = TopicPath::new("home/kitchen/temperature");
let subscribers = router.get_subscribers(&topic);

println!("Message 'home/kitchen/temperature' matches {} subscribers:", subscribers.len());
for (sub_id, (pattern, _qos), handler) in subscribers {
    println!("  - {} (pattern: {})", handler, pattern.topic_pattern());
}

/* Output:
Message 'home/kitchen/temperature' matches 5 subscribers:
  - exact_handler (pattern: home/kitchen/temperature)
  - any_room_temp_handler (pattern: home/+/temperature)
  - kitchen_all_sensors_handler (pattern: home/kitchen/+)
  - all_home_handler (pattern: home/#)
  - all_kitchen_subtopics_handler (pattern: +/kitchen/#)
*/
```

### Integration with MQTT Clients

#### With rumqttc

```rust
use mqtt_topic_engine::{TopicRouter, TopicPatternPath, TopicPath, CacheStrategy};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup MQTT client
    let mqtt_options = MqttOptions::new("test-client", "broker.hivemq.com", 1883);
    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
    
    // Setup topic router
    let mut router = TopicRouter::<String>::new();
    
    // Add subscription with automatic broker subscription
    let pattern = TopicPatternPath::new_from_string("sensors/+/data", CacheStrategy::NoCache)?;
    let (needs_subscribe, _sub_id) = router.add_subscription(
        pattern.clone(),
        QoS::AtLeastOnce,
        "sensor_handler".to_string()
    );
    
    if needs_subscribe {
        client.subscribe(pattern.mqtt_pattern().as_ref(), QoS::AtLeastOnce).await?;
    }
    
    // Handle incoming messages
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                let topic = TopicPath::new(publish.topic.as_str());
                let subscribers = router.get_subscribers(&topic);
                
                for (_sub_id, (pattern, _qos), handler) in subscribers {
                    println!("Handler '{}' received message on pattern: {}", 
                        handler, pattern.topic_pattern());
                    
                    // Route to appropriate handler
                    // process_message(handler, &publish.payload);
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        }
    }
    
    Ok(())
}
```

## Use Cases

### IoT Device Management

```rust
use mqtt_topic_engine::{TopicPatternPath, CacheStrategy};

// Subscribe to all sensors in a specific location
let pattern = TopicPatternPath::new_from_string(
    "iot/{building}/{floor}/sensors/{sensor_type}/{sensor_id}/telemetry",
    CacheStrategy::NoCache
)?;

let filtered = pattern
    .bind_parameter("building", "headquarters")?
    .bind_parameter("floor", "3")?;

println!("{}", filtered.mqtt_pattern());
// Output: iot/headquarters/3/sensors/+/+/telemetry
```

### Log Aggregation

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Route logs by service and severity
let pattern = TopicPatternPath::new_from_string(
    "logs/{service}/{severity}/{details:#}",
    CacheStrategy::NoCache
)?;

let topic = TopicPath::new("logs/api/error/auth/invalid-token");
let topic_match = pattern.try_match(topic)?;

let service = topic_match.get_named_param("service").unwrap();
let severity = topic_match.get_named_param("severity").unwrap();
let details = topic_match.get_named_param("details").unwrap();

if severity == "error" {
    println!("ERROR in {}: {}", service, details);
}
```

### Multi-Tenant Applications

```rust
// Each tenant gets isolated topic space
let pattern = TopicPatternPath::new_from_string(
    "tenant/{tenant_id}/devices/{device_id}/events",
    CacheStrategy::NoCache
)?;

// Tenant A only sees their devices
let tenant_a_pattern = pattern.bind_parameter("tenant_id", "tenant-a")?;

// Tenant B only sees their devices
let tenant_b_pattern = pattern.bind_parameter("tenant_id", "tenant-b")?;
```

### Working with ArcStr

Under the hood, `mqtt-topic-engine` uses `ArcStr` for efficient string handling with cheap cloning.
You can pass regular `&str` or `String` values, and they will be converted automatically:

```rust
use mqtt_topic_engine::{TopicPath, TopicPatternPath, CacheStrategy};
use arcstr::ArcStr;

// All of these work - the API accepts impl Into<ArcStr>:
let topic1 = TopicPath::new("sensors/temp");                    // &str
let topic2 = TopicPath::new(String::from("sensors/temp"));      // String  
let topic3 = TopicPath::new(ArcStr::from("sensors/temp"));      // ArcStr directly

// For performance-critical code, reuse ArcStr to avoid allocations:
let topic_string = ArcStr::from("sensors/temperature");
let topic_a = TopicPath::new(topic_string.clone());  // Cheap clone (reference counted)
let topic_b = TopicPath::new(topic_string.clone());  // Another cheap clone
let topic_c = TopicPath::new(topic_string);          // Move (also cheap)
```

**When to use `ArcStr` directly:**
- ✅ Storing topic strings for reuse (cheap cloning via reference counting)
- ✅ High-frequency topic creation from the same string values
- ✅ Sharing topic strings across threads (ArcStr is Send + Sync)
- ❌ Simple one-off topic matching (just use `&str` - it's simpler)

## Configuration

### Cache Strategies

```rust
use mqtt_topic_engine::CacheStrategy;
use std::num::NonZeroUsize;

// No caching (minimal memory, recompute every match)
let no_cache = CacheStrategy::NoCache;

// LRU cache with 100 entries (balance memory/performance)
// Note: requires 'lru-cache' feature
#[cfg(feature = "lru-cache")]
let lru_100 = CacheStrategy::Lru(NonZeroUsize::new(100).unwrap());

// LRU cache with 10000 entries (high performance, more memory)
#[cfg(feature = "lru-cache")]
let lru_10k = CacheStrategy::Lru(NonZeroUsize::new(10000).unwrap());
```

**When to use caching:**
- ✅ Repeated matches to same topics (e.g., sensor data every second)
- ✅ High message throughput
- ✅ Complex patterns with multiple wildcards
- ❌ Unique topics (no cache benefit)
- ❌ Memory-constrained environments

### Minimal Configuration for Embedded Systems

For resource-constrained environments, use only the core pattern matching without routing or caching:

```toml
[dependencies]
mqtt-topic-engine = { version = "0.1.0", default-features = false }
```

```rust
use mqtt_topic_engine::{TopicPatternPath, TopicPath, CacheStrategy};

// Minimal footprint: only pattern validation and matching
let pattern = TopicPatternPath::new_from_string(
    "sensors/{location}/data",
    CacheStrategy::NoCache  // No LRU dependency
)?;

let topic = TopicPath::new("sensors/kitchen/data");
let topic_match = pattern.try_match(topic)?;

if let Some(location) = topic_match.get_named_param("location") {
    // Process message for specific location
}
```

**Benefits of minimal configuration:**
- ✅ Smallest binary size (~10KB additional code)
- ✅ No heap allocations for routing or caching
- ✅ Suitable for microcontrollers and embedded systems
- ✅ Still provides full pattern matching and parameter extraction

## Performance

Topic engine is designed for high-performance applications:

- **Matching:** O(n) where n = number of topic segments  
- **Routing:** O(m) where m = number of matching subscriptions
- **Memory:** Optimized with `arcstr` and `smallvec` for minimal allocations
- **Caching:** Optional LRU cache for repeated topic matches

**Expected performance characteristics:**
- Pattern parsing: ~1-2 μs per pattern (typical case: 3-5 segments)
- Topic matching: ~200-500 ns per match without cache
- Topic matching: ~20-50 ns with cache hit (LRU lookup)
- Router lookup: ~500 ns - 2 μs (depends on subscription count and matches)

> **Note:** Actual performance depends on pattern complexity, topic depth, and hardware.
> Always benchmark with your specific workload.

## Integration

### MQTT Client Support

The library includes its own QoS type and provides optional integration with popular MQTT clients through seamless QoS type conversions:

**Supported MQTT clients:**
- **rumqttc** - Enable with `features = ["rumqttc"]`
- **paho-mqtt** - Enable with `features = ["paho-mqtt"]`
- **ntex-mqtt** - Enable with `features = ["ntex-mqtt"]`
- **Zero-dependency** - Works standalone without any MQTT client (default)

```toml
# With rumqttc integration
mqtt-topic-engine = { version = "0.1.0", features = ["rumqttc"] }

# With paho-mqtt integration
mqtt-topic-engine = { version = "0.1.0", features = ["paho-mqtt"] }

# With ntex-mqtt integration
mqtt-topic-engine = { version = "0.1.0", features = ["ntex-mqtt"] }

# Zero-dependency (no features)
mqtt-topic-engine = "0.1.0"
```

The core matching and routing logic is already client-agnostic and works with any MQTT client library.

### QoS Type Conversions

When MQTT client integration features are enabled, the library provides automatic QoS type conversions:

```rust
use mqtt_topic_engine::QoS;

// From client-specific QoS to mqtt-topic-engine QoS
#[cfg(feature = "rumqttc")]
let qos: QoS = rumqttc::QoS::AtLeastOnce.into();

#[cfg(feature = "paho-mqtt")]
let qos: QoS = paho_mqtt::QoS::AtLeastOnce.into();

#[cfg(feature = "ntex-mqtt")]
let qos: QoS = ntex_mqtt::QoS::AtLeastOnce.into();

// To client-specific QoS from mqtt-topic-engine QoS
let engine_qos = QoS::AtLeastOnce;

#[cfg(feature = "rumqttc")]
let rumqttc_qos: rumqttc::QoS = engine_qos.to_rumqttc();

#[cfg(feature = "paho-mqtt")]
let paho_qos: i32 = engine_qos.to_paho_mqtt();

#[cfg(feature = "ntex-mqtt")]
let ntex_qos: ntex_mqtt::QoS = engine_qos.to_ntex_mqtt();
```

This enables seamless integration with any supported MQTT client without manual QoS type conversions.

## API Documentation

For detailed API documentation, visit [docs.rs/mqtt-topic-engine](https://docs.rs/mqtt-topic-engine).

## Contributing

Contributions are welcome! This library is part of the [mqtt-typed-client](https://github.com/holovskyi/mqtt-typed-client) project.

## License

This project is licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## See Also

- [mqtt-typed-client](https://github.com/holovskyi/mqtt-typed-client) - Type-safe MQTT client using this engine
- [rumqttc](https://github.com/bytebeamio/rumqtt) - Async MQTT client for Rust
- [MQTT Specification](https://mqtt.org/) - Official MQTT protocol documentation
