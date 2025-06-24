//! Message serialization traits and implementations.

use std::fmt::Debug;

use bincode::{Decode, Encode};

/// Trait for serializing and deserializing MQTT message payloads.
///
/// Implement this trait to use custom serialization formats.
pub trait MessageSerializer<T>:
	Default + Clone + Send + Sync + 'static
{
	/// Error type for serialization failures
	type SerializeError: Debug + Send + Sync + 'static;
	/// Error type for deserialization failures
	type DeserializeError: Debug + Send + Sync + 'static;

	/// Convert data to bytes for MQTT transmission
	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError>;
	/// Convert bytes from MQTT into typed data
	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError>;
}

/// Default serializer using bincode format.
///
/// Requires types to implement `bincode::Encode` and `bincode::Decode`.
#[derive(Clone, Default)]
pub struct BincodeSerializer {
	config: bincode::config::Configuration,
}

impl BincodeSerializer {
	/// Creates a new serializer with default configuration.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a serializer with custom bincode configuration.
	pub fn with_config(config: bincode::config::Configuration) -> Self {
		Self { config }
	}
}

impl<T> MessageSerializer<T> for BincodeSerializer
where T: Encode + Decode<()> + 'static
{
	type SerializeError = bincode::error::EncodeError;
	type DeserializeError = bincode::error::DecodeError;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		bincode::encode_to_vec(data, self.config)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		bincode::decode_from_slice(bytes, self.config).map(|(value, _)| value)
	}
}
