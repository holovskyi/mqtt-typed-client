# Roadmap

*This document outlines the long-term vision for mqtt-typed-client development. Items are grouped by theme rather than strict version numbers.*

*Deep-dive research backing several of these items (MQTT v5, ack correlation, `no_std`) lives in [FUTURE_WORK_RESEARCH.md](./FUTURE_WORK_RESEARCH.md).*

## Core Protocol Enhancements

- [ ] **Add retain, qos, dup flags to incoming message metadata**  
  Currently these flags are received from rumqttc but discarded in `async_client.rs/Ok(Incoming(Publish(p)))`. Adding them to message metadata will give users full control over message properties and enable proper handling of retained messages. Could be implemented through a `raw_data` field in topic structure similar to the auto-populated `topic: Arc<TopicMatch>` field.

- [ ] **Add subscription acknowledgment confirmation**  
  Currently `subscribe()` doesn't analyze subscription results — `SubAck.return_codes`
  (where the broker can reject a subscription or downgrade QoS) are silently dropped.
  Minimal surfacing needs no rumqttc changes; reliable per-request correlation requires
  a fork (source-verified 2026-07-08, see FUTURE_WORK_RESEARCH.md §2 — subscribe pkid
  reuse makes fork-free correlation unsound under load).

- [ ] **Add publish acknowledgment confirmation**  
  Currently we don't know when the broker has actually confirmed message publication
  (according to QoS level). Decision: thin fork of rumqttc carrying an ack-notification
  channel (source-verified analysis in FUTURE_WORK_RESEARCH.md §2; a fork-free
  pkid-aware wrapper is possible for publishes but fragile).

## Performance & Optimization

- [ ] **Smart protocol compression support**  
  Implement adaptive compression based on message type and size. Use compositional approach separating serialization and compression layers for maximum flexibility.

- [ ] **Advanced backpressure handling is planned for future releases**

## Architecture Improvements

- [ ] **Create minimal library version for embedded devices**  
  Possibly with no_std mode support.

- [ ] **Consider using other low-level MQTT libraries**  
  Explore alternatives to rumqttc for different use cases.