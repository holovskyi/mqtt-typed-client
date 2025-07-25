# Roadmap

*This document outlines the long-term vision for mqtt-typed-client development. Items are grouped by theme rather than strict version numbers.*

## Core Protocol Enhancements

- [ ] **Add retain, qos, dup flags to incoming message metadata**  
  Currently these flags are received from rumqttc but discarded in `async_client.rs/Ok(Incoming(Publish(p)))`. Adding them to message metadata will give users full control over message properties and enable proper handling of retained messages. Could be implemented through a `raw_data` field in topic structure similar to the auto-populated `topic: Arc<TopicMatch>` field.

- [ ] **Add subscription acknowledgment confirmation**  
  Currently `subscribe()` doesn't analyze subscription results. Implement proper MQTT acknowledgment handling by tracking packet IDs from `Outgoing(Subscribe(1))` and matching them with `Incoming(SubAck())` responses. This may require forking rumqttc to change `Outgoing::Subscribe(u16)` to include full subscription details.

- [ ] **Add publish acknowledgment confirmation**  
  Currently we don't know when the broker has actually confirmed message publication (according to QoS level). Implement proper publish acknowledgment handling by tracking packet IDs and matching them with corresponding ACK responses. This will also require forking rumqttc.

## Performance & Optimization

- [ ] **Smart protocol compression support**  
  Implement adaptive compression based on message type and size. Use compositional approach separating serialization and compression layers for maximum flexibility.

## Architecture Improvements

- [ ] **Create minimal library version for embedded devices**  
  Possibly with no_std mode support.

- [ ] **Consider using other low-level MQTT libraries**  
  Explore alternatives to rumqttc for different use cases.