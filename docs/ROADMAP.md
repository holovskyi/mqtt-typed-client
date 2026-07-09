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

- [ ] **Ergonomic consumption APIs on top of the pull loop** (post-0.3)  
  The core primitive stays the pull loop (`receive().await`) — it composes with
  `select!`, cancellation, backpressure, and stack-local state. On top of it,
  consider (in this order of preference):
  1. **`Stream` adapter** — `subscriber -> impl futures::Stream<Item = …>`,
     giving the whole `StreamExt` toolbox (`for_each`, `filter_map`,
     `buffered(n)` for *controlled* concurrency) and idiomatic async ergonomics.
  2. **Callback subscription** — thin sugar only: ONE task per subscription
     awaiting the handler **sequentially** (preserves the per-subscriber FIFO
     guarantee), returning a guard/handle to stop it. **Never spawn a task
     per message** — that reintroduces the slow-consumer reordering bug fixed in
     0.3 §5, plus unbounded concurrency. Callbacks also make it easy to silently
     ignore `Err`/lag notices and push users toward `Arc<Mutex<>>` for shared
     state, so they rank below the `Stream` adapter.
  Decision pending; revisit after the 0.3 receive-API shape (lag/deserialize
  surfacing) settles.

- [ ] **Create minimal library version for embedded devices**  
  Possibly with no_std mode support.

- [ ] **Consider using other low-level MQTT libraries**  
  Explore alternatives to rumqttc for different use cases.