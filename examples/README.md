# Examples Guide

This directory contains examples demonstrating how to use the `mqtt-typed-client` library. Each example showcases different features and use cases.

## 🚀 Quick Start

1. **Start local MQTT broker:**
   ```bash
   cd dev
   docker-compose up -d
   ```

2. **Run your first example:**
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

### 🌟 **000_hello_world.rs** - Basic Usage
**What it demonstrates:**
- Basic publish/subscribe pattern
- Topic parameter extraction with `#[mqtt_topic]` macro
- Type-safe message routing
- Automatic serialization/deserialization

**Key concepts:**
- Topic pattern: `greetings/{language}/{sender}`
- BincodeSerializer for efficient binary serialization
- Wildcard subscriptions: `greetings/+/+`

### 🏓 **001_ping_pong.rs** - Multi-Client Communication  
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

### ⚙️ **002_configuration.rs** - Advanced Configuration
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

### 🔒 **004_hello_world_tls.rs** - TLS/SSL Connections
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
MQTT_BROKER=\"mqtt://broker.hivemq.com:1883\" cargo run --example 000_hello_world
```

## 🎯 Learning Path

**Recommended order for learning:**

1. **000_hello_world.rs** - Start here to understand basics
2. **002_configuration.rs** - Learn about client configuration  
3. **001_ping_pong.rs** - See multi-client patterns
4. **004_hello_world_tls.rs** - Add security with TLS

## 🔧 Troubleshooting

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
2. Use plain MQTT: MQTT_BROKER=\"mqtt://localhost:1883\"
3. Try public broker: MQTT_BROKER=\"mqtt://broker.hivemq.com:1883\"
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
MQTT_BROKER=\"mqtt://broker.hivemq.com:1883\" cargo run --example 000_hello_world
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
    let connection_url = shared::config::build_url(\"example_name\");
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

1. **Copy template from 000_hello_world.rs**
2. **Update client_id prefix** in `build_url(\"your_example_name\")`
3. **Add your MQTT logic**
4. **Test with local broker**
5. **Update this README with your example**

## 📖 Additional Resources

- **Main documentation:** [../README.md](../README.md)
- **API reference:** `cargo doc --open`
- **MQTT protocol:** [mqtt.org](https://mqtt.org/)
- **Local broker management:** [dev/README.md](../dev/README.md)

---

**Happy coding!** 🦀 If you run into issues, check the troubleshooting section above or create an issue on GitHub.
