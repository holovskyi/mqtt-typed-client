# mqtt-typed-client-doc-macros

⚠️ **Internal use only**

This crate contains procedural macros used internally by `mqtt-typed-client` 
for documentation generation. It is NOT intended for direct use.

If you're looking for the MQTT client library, see:
- [mqtt-typed-client](https://crates.io/crates/mqtt-typed-client) - Main library

## Purpose

This crate transforms Markdown documentation at compile-time to adapt 
GitHub-style links for rustdoc rendering.

## Why a separate crate?

This proc-macro is separated from the main `mqtt-typed-client-macros` to:
- Keep domain logic (MQTT) separate from infrastructure (documentation)
- Follow Single Responsibility Principle
- Make it clear this is internal tooling

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
