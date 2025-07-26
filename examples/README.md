# Examples Guide

This directory contains examples demonstrating how to use the `mqtt-typed-client` library. Each example showcases different features and use cases.

## 📋 Quick Navigation

Jump directly to any example:

- **[000_hello_world](#000_hello_worldrs---basic-usage)** - Basic Usage
- **[001_ping_pong](#001_ping_pongrs---multi-client-communication)** - Multi-Client Communication
- **[002_configuration](#⚙️-002_configurationrs---advanced-configuration)** - Advanced Configuration
- **[003_hello_world_lwt](#003_hello_world_lwtrs---last-will--testament-lwt)** - Last Will & Testament
- **[004_hello_world_tls](#🔒-004_hello_world_tlsrs---tlsssl-connections)** - TLS/SSL Connections
- **[005_hello_world_serializers](#🔧-005_hello_world_serializersrs---custom-message-serializers)** - Custom Serializers
- **[006_retain_and_clear](#🔄-006_retain_and_clearrs---mqtt-retained-messages)** - Retained Messages
- **[007_custom_patterns](#007_custom_patternsrs---custom-topic-patterns)** - Custom Topic Patterns
- **[008_modular_example](#008_modular_examplers---modular-project-architecture)** - Modular Architecture
- **[100_all_serializers_demo](#100_all_serializers_demors---complete-serializer-test-suite)** - Complete Serializer Test Suite

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

### <a id="000_hello_worldrs---basic-usage"></a>**[000_hello_world.rs](example_000_hello_world/index.html)** - Basic Usage
**What it demonstrates:**
- Basic publish/subscribe pattern
- Topic parameter extraction with `#[mqtt_topic]` macro
- Type-safe message routing
- Automatic serialization/deserialization

**Key concepts:**
- Topic pattern: `greetings/{language}/{sender}`
- BincodeSerializer for efficient binary serialization
- Wildcard subscriptions: `greetings/+/+`

### <a id="001_ping_pongrs---multi-client-communication"></a>**[001_ping_pong.rs](example_001_ping_pong/index.html)** - Multi-Client Communication  
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

### <a id="⚙️-002_configurationrs---advanced-configuration"></a>⚙️ **[002_configuration.rs](example_002_configuration/index.html)** - Advanced Configuration
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

### <a id="003_hello_world_lwtrs---last-will--testament-lwt"></a>**[003_hello_world_lwt.rs](example_003_hello_world_lwt/index.html)** - Last Will & Testament (LWT)
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

### <a id="🔒-004_hello_world_tlsrs---tlsssl-connections"></a>🔒 **[004_hello_world_tls.rs](example_004_hello_world_tls/index.html)** - TLS/SSL Connections
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

### <a id="🔧-005_hello_world_serializersrs---custom-message-serializers"></a>🔧 **[005_hello_world_serializers.rs](example_005_hello_world_serializers/index.html)** - Custom Message Serializers
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

### <a id="🔄-006_retain_and_clearrs---mqtt-retained-messages"></a>🔄 **[006_retain_and_clear.rs](example_006_retain_and_clear/index.html)** - MQTT Retained Messages
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

### <a id="007_custom_patternsrs---custom-topic-patterns"></a>**[007_custom_patterns.rs](example_007_custom_patterns/index.html)** - Custom Topic Patterns
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
```rust
// Standard usage:
topic_client.subscribe().await?
topic_client.publish("rust", "alice", &msg).await?
GreetingTopic::last_will("rust", "client", msg)

// Custom patterns:
topic_client.subscription().with_pattern("dev/greetings/{language}/{sender}")?.subscribe().await?
topic_client.get_publisher_to("dev/greetings/{language}/{sender}", "rust", "alice")?.publish(&msg).await?
GreetingTopic::last_will_to("dev/greetings/{language}/{sender}", "rust", "client", msg)?
```

### <a id="008_modular_examplers---modular-project-architecture"></a>**[008_modular_example.rs](example_008_modular_example/index.html)** - Modular Project Architecture
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

### <a id="100_all_serializers_demors---complete-serializer-test-suite"></a>**[100_all_serializers_demo.rs](example_100_all_serializers_demo/index.html)** - Complete Serializer Test Suite
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

## 🎯 Learning Path

**Recommended order for learning:**

1. **[000_hello_world.rs](example_000_hello_world/index.html)** - Start here to understand basics
2. **[001_ping_pong.rs](example_001_ping_pong/index.html)** - See multi-client patterns
3. **[002_configuration.rs](example_002_configuration/index.html)** - Learn about client configuration  
4. **[003_hello_world_lwt.rs](example_003_hello_world_lwt/index.html)** - Understand MQTT reliability features
5. **[004_hello_world_tls.rs](example_004_hello_world_tls/index.html)** - Add security with TLS
6. **[005_hello_world_serializers.rs](example_005_hello_world_serializers/index.html)** - Custom message serialization
7. **[006_retain_and_clear.rs](example_006_retain_and_clear/index.html)** - MQTT retained messages and broker storage
8. **[007_custom_patterns.rs](example_007_custom_patterns/index.html)** - Override default topic patterns for advanced routing
9. **[008_modular_example.rs](example_008_modular_example/index.html)** - Organize complex applications with modular architecture
10. **[100_all_serializers_demo.rs](example_100_all_serializers_demo/index.html)** - Complete serialization ecosystem test

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

```rust
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

- **Main documentation:** [../README.md](../README.md)
- **API reference:** `cargo doc --open`
- **MQTT protocol:** [mqtt.org](https://mqtt.org/)
- **Local broker management:** [dev/README.md](../dev/README.md)

---

**Happy coding!** 🦀 If you run into issues, check the troubleshooting section above or create an issue on GitHub.
