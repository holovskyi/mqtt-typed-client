# MQTT Typed Client Macros

Procedural macros for type-safe async MQTT client with automatic topic routing.

This crate provides the `#[mqtt_topic]` macro that generates type-safe MQTT topic handling code.

## Usage

This crate is typically used through the main `mqtt-typed-client` crate. The macro is re-exported for convenient access.

```rust
use mqtt_typed_client_macros::mqtt_topic;

#[mqtt_topic("sensors/{sensor_id}/data")]
struct SensorData {
    sensor_id: String,
    payload: MyData,
}
```

The macro generates:
- Subscription and publishing methods
- Topic parameter extraction
- Type-safe topic pattern validation
- Integration with MQTT client

See the main [mqtt-typed-client](https://crates.io/crates/mqtt-typed-client) crate for complete examples and documentation.

## License

This project is licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
