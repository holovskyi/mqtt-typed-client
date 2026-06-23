# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Internal documentation macros for
[`mqtt-typed-client`](https://crates.io/crates/mqtt-typed-client) (not intended
for direct use).

## [Unreleased]

## [0.1.0] - 2026-06-23

First release. Extracted from the `mqtt-typed-client` build script.

### Added
- `include_md_transformed!` proc-macro that embeds a Markdown file as crate-level
  documentation, transforming README links for docs.rs. Replaces the previous
  `build.rs`-based approach (supply-chain hardening — no more build script).
