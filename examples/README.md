# Examples Guide

<!--
DEVELOPMENT NOTE:
This file is embedded into the crate docs via `include_str!` (see src/lib.rs).
There is no link transform anymore. Example sources are linked by reference-style
shortcuts: write `[NNN_name.rs]` inline, and define the URL once in the link-
definition block at the BOTTOM of this file. Relative links (`[foo.rs](foo.rs)`)
render broken on docs.rs — always use the absolute GitHub URL in the definition.
-->

This directory contains examples demonstrating how to use the `mqtt-typed-client` library. Each example showcases different features and use cases.

## 📋 Quick Navigation

Jump directly to any example:

- **[000_hello_world.rs]** - Basic Usage
- **[001_ping_pong.rs]** - Multi-Client Communication
- **[002_configuration.rs]** - Advanced Configuration
- **[003_hello_world_lwt.rs]** - Last Will & Testament
- **[004_hello_world_tls.rs]** - TLS/SSL Connections
- **[005_hello_world_serializers.rs]** - Custom Serializers
- **[006_retain_and_clear.rs]** - Retained Messages
- **[007_custom_patterns.rs]** - Custom Topic Patterns
- **[008_modular_example.rs]** - Modular Architecture
- **[009_message_metadata.rs]** - Message Metadata
- **[100_all_serializers_demo.rs]** - Complete Serializer Test Suite
- **[102_multi_serializer_macro.rs]** - Per-Topic Custom Serializers

## 🚀 Quick Start

**⚠️ Important:** Always run examples from the project root directory!

1. **Start local MQTT broker:**
   ```bash
   cd dev
   docker-compose up -d
   cd ..  # Return to project root
   ```

2. **Run your first example (from project root):**
   ```bash
   cargo run --example 000_hello_world
   ```

3. **Enable logging (optional):**
   ```bash
   # Edit examples/.env and uncomment:
   # RUST_LOG=info
   
   # Or set temporarily:
   RUST_LOG=info cargo run --example 000_hello_world
   ```

## 📚 Examples Index

### <a id="000_hello_worldrs---basic-usage"></a>**[000_hello_world.rs]** - Basic Usage
**What it demonstrates:**
- Basic publish/subscribe pattern
- Topic parameter extraction with `#[mqtt_topic]` macro
- Type-safe message routing
- Automatic serialization/deserialization

**Key concepts:**
- Topic pattern: `greetings/{language}/{sender}`
- BincodeSerializer for efficient binary serialization
- Wildcard subscriptions: `greetings/+/+`

### <a id="001_ping_pongrs---multi-client-communication"></a>**[001_ping_pong.rs]** - Multi-Client Communication  
**What it demonstrates:**
- Multiple MQTT clients in one application
- Inter-client communication patterns
- Game state management over MQTT
- Concurrent async operations

**Key concepts:**
- Client cloning and sharing
- Topic-based game logic
- Random event generation
- Graceful shutdown handling

### <a id="⚙️-002_configurationrs---advanced-configuration"></a>⚙️ **[002_configuration.rs]** - Advanced Configuration
**What it demonstrates:**
- Custom MQTT client settings
- Connection parameter tuning
- Quality of Service (QoS) levels
- Keep-alive and session management

**Key concepts:**
- MqttClientConfig customization
- Connection timeouts and retries
- Cache size optimization
- Credential management

### <a id="003_hello_world_lwtrs---last-will--testament-lwt"></a>**[003_hello_world_lwt.rs]** - Last Will & Testament (LWT)
**What it demonstrates:**
- MQTT Last Will & Testament functionality
- Ungraceful vs graceful disconnect handling
- Two separate client connections (subscriber/publisher)
- LWT message configuration and triggering

**Key concepts:**
- LWT configuration with typed topics
- Unexpected disconnect simulation
- Message differentiation (normal vs LWT)
- Multi-terminal example usage

**Usage:**
```bash
# Terminal 1: Start subscriber
cargo run --example 003_hello_world_lwt

# Terminal 2: Run publisher (sends greeting then crashes)
cargo run --example 003_hello_world_lwt -- --publisher
```

### <a id="🔒-004_hello_world_tlsrs---tlsssl-connections"></a>🔒 **[004_hello_world_tls.rs]** - TLS/SSL Connections
**What it demonstrates:**
- Secure MQTT connections (MQTTS)
- Custom TLS certificate handling
- Self-signed certificate setup
- TLS configuration for development

**Key concepts:**
- rustls integration
- Certificate validation
- TLS transport configuration
- Development vs production certificates

### <a id="🔧-005_hello_world_serializersrs---custom-message-serializers"></a>🔧 **[005_hello_world_serializers.rs]** - Custom Message Serializers
**What it demonstrates:**
- Using different serializers (MessagePack vs built-in Bincode)
- Creating custom MessageSerializer trait implementation
- Easy serializer switching with minimal code changes
- Same MQTT logic with different data formats

**Key concepts:**
- MessageSerializer trait implementation
- serde::DeserializeOwned requirement
- Binary vs text serialization formats
- Custom serializer wrapper creation

### <a id="🔄-006_retain_and_clearrs---mqtt-retained-messages"></a>🔄 **[006_retain_and_clear.rs]** - MQTT Retained Messages
**What it demonstrates:**
- MQTT retained message functionality with multiple clients
- Message persistence and broker storage behavior
- Retained message replacement and clearing
- Different connection timing scenarios

**Key concepts:**
- Retained vs non-retained messages
- Broker message storage and delivery to new subscribers
- `clear_retained()` functionality with empty payloads
- Multi-client demonstration with timing coordination
- Automatic empty payload filtering

**Timeline demonstration:**
- t=0s: First retained message stored by broker
- t=1s: Subscriber-1 connects → receives retained message #1
- t=5s: Second retained message replaces first in broker storage
- t=6s: Subscriber-2 connects → receives retained message #1 (timing)
- t=10s: Non-retained message → only active subscribers receive
- t=11s: Subscriber-3 connects → receives retained message #2
- t=15s: Clear retained messages from broker storage
- t=18s: Subscriber-4 connects → receives nothing (storage empty)

### <a id="007_custom_patternsrs---custom-topic-patterns"></a>**[007_custom_patterns.rs]** - Custom Topic Patterns
**What it demonstrates:**
- Overriding default topic patterns from `#[mqtt_topic]` macro
- Environment-specific topic routing (dev/prod prefixes)
- Pattern compatibility validation and type safety
- Advanced subscription, publishing, and Last Will configuration

**Key concepts:**
- `.with_pattern()` for custom subscription patterns
- `.get_publisher_to()` for custom publishing patterns
- `.last_will_to()` for custom Last Will patterns
- Pattern compatibility rules (same parameters, names, order)
- Multi-tenant and environment isolation patterns

**API comparison:**
```rust,ignore
// Standard usage:
topic_client.subscribe().await?
topic_client.publish("rust", "alice", &msg).await?
GreetingTopic::last_will("rust", "client", msg)

// Custom patterns:
topic_client.subscription().with_pattern("dev/greetings/{language}/{sender}")?.subscribe().await?
topic_client.get_publisher_to("dev/greetings/{language}/{sender}", "rust", "alice")?.publish(&msg).await?
GreetingTopic::last_will_to("dev/greetings/{language}/{sender}", "rust", "client", msg)?
```

### <a id="008_modular_examplers---modular-project-architecture"></a>**[008_modular_example.rs]** - Modular Project Architecture
**What it demonstrates:**
- Organizing MQTT applications with multiple modules
- Separating topic definitions from business logic
- Clean import patterns and code organization
- Multiple subscription strategies (wildcard vs. filtered)
- Production-ready error handling and graceful shutdown

**Key concepts:**
- Modular topic definitions in separate files
- Type-safe MQTT operations with custom data structures
- Subscription patterns: wildcard (`sensors/+/+/+/data`) vs. specific filters
- Publisher/subscriber coordination with proper timing
- Structured logging and timeout handling

**Project structure:**
```text
modular_example/
├── mod.rs          # Module exports
├── topics.rs       # Topic definitions and data structures  
└── runner.rs       # Business logic and execution flow
```

**Topic pattern:** `sensors/{location}/{sensor_type}/{device_id}/data`
- **Publishing:** `client.temperature_topic().get_publisher("Home", "floor", 37)`
- **Wildcard subscription:** Receives from all sensors
- **Filtered subscription:** Only specific device with caching
- **Data flow:** Real sensor data → MQTT → Multiple typed subscribers

### <a id="009_message_metadatars---message-metadata"></a>**[009_message_metadata.rs]** - Message Metadata
**What it demonstrates:**
- The optional `meta` field: per-message QoS, retain, and dup flags
- The optional `topic` field: the concrete matched topic (wildcards resolved)
- Arc-adaptive field forms: bare `MessageMeta` / `TopicMatch` (owned) vs
  `Arc<...>` (zero-copy, recommended for hot paths)

**Topic pattern:** `telemetry/{device}`

### <a id="100_all_serializers_demors---complete-serializer-test-suite"></a>**[100_all_serializers_demo.rs]** - Complete Serializer Test Suite
**What it demonstrates:**
- Full publish/subscribe cycle testing for all 8 available serializers
- Serialization and deserialization verification
- Connection testing for schema-based serializers (Protobuf)
- Comprehensive coverage of the entire serialization architecture

**Available serializers tested:**
- **Serde-compatible:** JSON, MessagePack, CBOR, Postcard, RON, Flexbuffers
- **Bincode:** Native Rust binary format
- **Schema-based:** Protobuf (connection-only, requires generated types)

**Key concepts:**
- Complete serialization ecosystem demonstration
- Feature flag system (requires --all-features)
- Real-world publish/subscribe verification
- Error handling and diagnostics

**Usage:**
```bash
# Requires all serializer features to be enabled
cargo run --example 100_all_serializers_demo --all-features
```

### <a id="102_multi_serializer_macrors---per-topic-custom-serializers"></a>**[102_multi_serializer_macro.rs]** - Per-Topic Custom Serializers
**What it demonstrates:**
- Using different serializers for different topics in the same MQTT session
- Custom serializer specification via `#[mqtt_topic]` macro attribute
- Integration with legacy systems requiring specific formats (JSON)
- Modern topics using efficient binary formats (Bincode)
- Shared MQTT connection with multiple serialization strategies

**Key concepts:**
- `serializer = JsonSerializer` attribute syntax
- Default serializer from client vs. per-topic overrides
- `clone_with_serializer()` internal mechanism
- Mixed serialization formats in one application
- Type safety with different payload formats

**Use cases:**
- Legacy system integration (existing systems expect JSON)
- Performance optimization (binary for high-frequency, JSON for debugging)
- Protocol interoperability (external services with format requirements)
- IoT heterogeneous systems (different device capabilities)

**Example patterns:**
```rust,ignore
// Modern binary format (default from client)
#[mqtt_topic("v2/sensors/{id}/data")]
struct ModernSensor { id: u32, payload: SensorData }

// Legacy JSON format (custom serializer)
#[mqtt_topic("legacy/devices/{id}/status", serializer = JsonSerializer)]
struct LegacyDevice { id: String, payload: Status }

// Same client, different formats
let (client, conn) = MqttClient::<BincodeSerializer>::connect(url).await?;
let modern_sub = ModernSensor::subscribe(&client).await?;  // Uses Bincode
let legacy_sub = LegacyDevice::subscribe(&client).await?;  // Uses JSON
```

**Limitations:**
- TypedClient generation disabled for custom serializers (technical limitation)
- Custom serializer must be `Default + Clone + Send + Sync + 'static`
- Direct methods only: `LegacyDevice::subscribe(&client)` instead of `client.legacy_device()`

## 🛠️ Configuration

Examples use configuration files for easy setup:

### `.env` File (committed, safe defaults)
```bash
# MQTT Broker
MQTT_BROKER=mqtt://localhost:1883

# Tracing (uncomment to enable)
# RUST_LOG=info     # Recommended for learning
# RUST_LOG=debug    # Shows all MQTT traffic
```

### `.env.local` File (ignored by git, for secrets)
```bash
# Create this file for sensitive configuration
MQTT_USERNAME=your_username
MQTT_PASSWORD=your_password
```

### Environment Variables
You can override any setting:
```bash
MQTT_BROKER="mqtt://broker.hivemq.com:1883" cargo run --example 000_hello_world
```

## 🔧 Troubleshooting

### ⚠️ Wrong Working Directory

**Error:** `No such file or directory: examples/.env` or config not loading
```bash
# Solution: Make sure you're in the project root directory
pwd  # Should show: /path/to/mqtt-typed-client
ls   # Should show: Cargo.toml, examples/, dev/, etc.

# If you're in examples/ directory:
cd ..

# Then run examples:
cargo run --example 000_hello_world
```

### Connection Issues

**Error:** `Connection failed to mqtt://localhost:1883`
```bash
# Solution: Start the local broker
cd dev && docker-compose up -d
```

**Error:** `TLS handshake failed`
```bash
# Solutions:
1. Check if TLS broker is running: docker-compose ps
2. Use plain MQTT: MQTT_BROKER="mqtt://localhost:1883"
3. Try public broker: MQTT_BROKER="mqtt://broker.hivemq.com:1883"
```

### No Logging Output
```bash
# Enable logging by uncommenting in examples/.env:
RUST_LOG=info

# Or set temporarily:
RUST_LOG=debug cargo run --example 000_hello_world
```

### Port Conflicts
```bash
# If ports 1883/8883 are in use, modify dev/docker-compose.yml
# Or use external broker:
MQTT_BROKER="mqtt://broker.hivemq.com:1883" cargo run --example 000_hello_world
```

## 🧩 Code Structure

All examples use the shared helper modules:

```rust,ignore
mod shared;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing from .env configuration
    shared::tracing::setup(None);
    
    // Connect with automatic URL and client_id generation
    let connection_url = shared::config::build_url("example_name");
    let (client, connection) = MqttClient::<BincodeSerializer>::connect(&connection_url)
        .await
        .inspect_err(|e| {
            shared::config::print_connection_error(&connection_url, e);
        })?;
    
    // Your example code here...
    
    connection.shutdown().await?;
    Ok(())
}
```

## 🚀 Creating New Examples

**⚠️ Remember:** Examples must be run from project root directory!

1. **Copy template from 000_hello_world.rs**
2. **Update client_id prefix** in `build_url("your_example_name")`
3. **Add your MQTT logic**
4. **Test from project root:** `cargo run --example your_example_name`
5. **Update this README with your example**

## 📖 Additional Resources

- **Main documentation:** [README.md](https://github.com/holovskyi/mqtt-typed-client/blob/main/README.md)
- **API reference:** `cargo doc --open`
- **MQTT protocol:** [mqtt.org](https://mqtt.org/)
- **Local broker management:** [dev/README.md](https://github.com/holovskyi/mqtt-typed-client/blob/main/dev/README.md)

---

**Happy coding!** 🦀 If you run into issues, check the troubleshooting section above or create an issue on GitHub.

<!--
Example source links — single definition per file (referenced from both the
Quick Navigation list and the Examples Index headers above). When adding a new
example, write `[NNN_name.rs]` where you mention it and add ONE matching line
here. Must be a full absolute GitHub URL (no link transform on docs.rs).
-->
[000_hello_world.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/000_hello_world.rs
[001_ping_pong.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/001_ping_pong.rs
[002_configuration.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/002_configuration.rs
[003_hello_world_lwt.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/003_hello_world_lwt.rs
[004_hello_world_tls.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/004_hello_world_tls.rs
[005_hello_world_serializers.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/005_hello_world_serializers.rs
[006_retain_and_clear.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/006_retain_and_clear.rs
[007_custom_patterns.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/007_custom_patterns.rs
[008_modular_example.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/008_modular_example.rs
[009_message_metadata.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/009_message_metadata.rs
[100_all_serializers_demo.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/100_all_serializers_demo.rs
[102_multi_serializer_macro.rs]: https://github.com/holovskyi/mqtt-typed-client/blob/main/examples/102_multi_serializer_macro.rs
