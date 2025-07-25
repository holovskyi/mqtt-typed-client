//! # Modular MQTT Client Example
//!
//! Demonstrates how to structure a multi-module MQTT application
//! with typed topics and organized imports.
//!
//! Key features showcased:
//! - Modular topic definitions separated from business logic
//! - Type-safe MQTT operations with custom data structures
//! - Multiple subscription patterns (wildcard vs. specific filters)
//! - Clean separation between configuration, topics, and execution logic
//! - Proper error handling and graceful shutdown
//!
//! Project structure:
//! ```
//! modular_example/
//! ├── mod.rs          # Module exports
//! ├── topics.rs       # Topic definitions and data structures
//! └── runner.rs       # Business logic and execution flow
//! ```
//!
//! Topic pattern: "sensors/{location}/{sensor_type}/{device_id}/data"
//! - Publishing: client.publish("Home", "floor", 37, &temp_data)
//! - Subscribing: All sensors vs. specific device filtering
//! - Message flow: Real sensor data → MQTT → Multiple subscribers

mod modular_example;
mod shared;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing - respects RUST_LOG environment variable
	shared::tracing::setup(None);

	println!("Starting Modular MQTT Client example...\n");

	// Run the modular example with proper error handling
	modular_example::run_example().await?;

	println!("\n- Example completed successfully!");
	Ok(())
}
