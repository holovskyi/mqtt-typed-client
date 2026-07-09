# fastmqtt / fastapi-mqtt — API ideas audit (Python ecosystem)

*2026-07-09. Question: does the Python "fastmqtt" niche contain API/DX ideas
worth borrowing for 0.3/0.4? Short answer: the libraries themselves are
dormant hobby/wrapper projects far behind us on typing — but fastmqtt's RPC
design is the best concrete request/response shape found in any survey so far,
and directly informs our 0.4 typed-RPC plan.*

## Identity

| | fastmqtt (toxazhl) | fastapi-mqtt (sabuhish) |
|---|---|---|
| What | standalone asyncio MQTT **v5** client over aiomqtt | FastAPI integration over gmqtt |
| Last push | 2024-08 (dormant) | 2024-05 (dormant) |
| Downloads/mo | ~53 (unused) | ~13,100 (real userbase) |
| Topic params typing | none | none |
| Matching | v5 subscription identifiers (drops messages if broker omits them) | linear string walk over all filters |

Both are strictly behind our macro approach on typing, param parsing, and
routing. Note the brand collision: fastapi-mqtt's main class is literally
named `FastMQTT`. The "Fast\*" prefix in Python signals the FastAPI
decorator-router idiom, not speed — the connotation does not transfer to Rust
(our equivalent idiom is axum/serde, which we already follow). No naming
action for us.

## Ideas worth stealing (ranked)

1. **RPC `ResponseContext` shape (fastmqtt `response.py`, ~120 lines) → 0.4
   typed RPC.** One long-lived subscription to a shared response topic per
   context, amortized over many requests; `request()` stamps
   `response_topic` + `correlation_data` (pluggable generator, default cycling
   counter), parks a future in a `map[correlation_data → future]`; the
   subscription callback resolves it; close/Drop cancels all pending.
   Rust mapping: RAII guard struct, `request()` → future, per-request timeout,
   oneshot map. Copy the guardrail too: **error if the caller pre-set
   `response_topic`/`correlation_data`** in publish properties.
2. **Responder auto-reply from handler return value.** If a handler returns
   non-None and the request carried `response_topic`, the framework publishes
   the return value there, echoing correlation data. With our macro this can
   be first-class: RPC handler variant whose typed return value IS the
   response. Best server-side RPC DX in any surveyed library.
3. **Dispatch by v5 subscription identifiers (0.4).** Not a trie replacement
   (fastmqtt's message-dropping when brokers omit identifiers shows why), but
   as a fast/exact path for handler attribution under overlapping filters,
   with the trie as fallback.
4. **Per-subscription v5 subscribe options + merge semantics.** `qos /
   no_local / retain_as_published / retain_handling` per topic with
   client-level defaults; explicit conflict rules when handlers share a
   filter (max QoS, hard error on flag mismatch). The merge/conflict rules
   are worth writing down when 0.4 subscribe options land.
5. **Raw payload reachable next to decoded `T`.** fastmqtt keeps
   `Payload.raw()` alongside lazy decode. Cheap and useful for
   logging/debugging; aligns with the 0.3 `MessageMeta` work.
6. **Lifecycle-hook demand signal.** fastapi-mqtt's 13k dl/mo with
   `on_connect/on_disconnect/on_subscribe` hooks confirms the 0.3
   connection-observability scope — users specifically want the SUBACK moment
   (granted QoS) surfaced. Their callback shape isn't worth copying; our
   watch-channel/event-stream design is the better spelling.

## Not transferable

Decorator registration + `MQTTRouter` (Python runtime-registration idiom; our
macro solves it at compile time — a router layer is YAGNI); client-global
encoder/decoder (worse than per-topic serializers); dict-state on the client;
client handle inside every message (in Rust: just clone the client);
fastapi-mqtt internals generally (monkeypatched gmqtt, untyped 5-arg
callbacks). Neither library has testing utilities — if anything, an opening
for us.

## Cross-links

- Fills the RPC-shape gap noted in
  [CLIENT_LIBRARY_LANDSCAPE_2026.md](./CLIENT_LIBRARY_LANDSCAPE_2026.md)
  (MQTTnet/paho.golang describe request/response only abstractly).
- Items 1–4 are 0.4 scope (see [../PLAN_0.3.md](../PLAN_0.3.md) out-of-scope
  list); items 5–6 reinforce existing 0.3 scope, no changes needed.
