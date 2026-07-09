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

- [ ] **Deserialize-once for identically-typed subscribers** (low priority)
  When N subscribers match the *same concrete topic* with the *same* payload
  type `T` and serializer `F`, the payload is deserialized N times. The routing
  actor is type-erased at `Bytes` (`Subscriber<Bytes>`), so each typed
  `MqttSubscriber<T, F>` deserializes independently — this is deliberate,
  because two subscribers on one topic may legitimately request *different*
  types (a broad wildcard vs a narrow one) and the actor cannot know `T`.
  A deserialize-once cache keyed by `(TypeId, serializer)` would save the
  redundant work only in the identical-`(T, F)` case, at the cost of type-aware
  caching in the hot path. Marginal payoff (deserialization is usually cheap;
  the overlap case is a minority), real complexity — hence low priority. If ever
  built, `payload` would become an `Arc<T>` shared across those subscribers, at
  which point the macro's Arc-vs-bare field-type adaptation (already used for
  `topic`/`meta`) would extend to `payload` for free.

## Architecture Improvements

- [ ] **Carry the concrete topic into `MessageConversionError`** (post-0.3, DX)
  On a wildcard/pattern subscription, a payload that fails to deserialize at the
  structured (macro) layer produces a `MessageConversionError` with no topic —
  so the user knows the *pattern* (`sensors/+/data`) but not the *concrete*
  topic (`sensors/broken/data`) the bad message arrived on. The concrete
  `Arc<TopicMatch>` IS available at the failure site (the mid-level
  `MqttSubscriber::receive` even exposes it as `ReceiveEvent::DecodeFailed(
  DecodeFailure { topic, meta, error })`); the structured layer just drops it
  when wrapping into `MessageConversionError::PayloadDeserializationError`. Fix
  properly = thread
  the topic through ALL `MessageConversionError` variants, including the
  `TopicParameterMissing`/`TopicParameterParseError` ones constructed inside the
  macro-generated `from_mqtt_message` — so it touches the `macros/` proc-macro
  codegen, not just the enum. Deliberately deferred from 0.3 §2b as a
  self-contained change; workaround today is the mid-level `client.subscribe::<T>`.

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

- [ ] **Migrate to edition 2024** (housekeeping, a future release)
  `core`, `macros`, and the workspace root are on edition 2021 but already
  carry MSRV 1.85.1 — which is exactly what edition 2024 requires — so the
  usual "stay on 2021 for a lower MSRV" reason no longer applies to them and
  the migration cost is near zero. No 2024 feature is needed today, so this is
  pure housekeeping (`cargo fix --edition` + review the temporary-scope / RPIT
  capture semantic changes), deferred to a future release. Keep
  `mqtt-topic-engine` on edition 2021 deliberately: it publishes standalone
  with MSRV 1.82, and a low MSRV has value for embedded/standalone consumers
  (2024 would force it up to 1.85).

- [ ] **Create minimal library version for embedded devices**  
  Possibly with no_std mode support.

- [ ] **Consider using other low-level MQTT libraries**  
  Explore alternatives to rumqttc for different use cases.