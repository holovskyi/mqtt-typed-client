//! Message serialization traits and implementations.

use std::fmt::Debug;

#[cfg(feature = "bincode-serializer")]
use bincode::{Decode, Encode};
use serde::{de::DeserializeOwned, Serialize};

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
/// 
/// Available when the `bincode-serializer` feature is enabled (default).
#[cfg(feature = "bincode-serializer")]
#[derive(Clone, Default)]
pub struct BincodeSerializer {
	config: bincode::config::Configuration,
}

#[cfg(feature = "bincode-serializer")]
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

#[cfg(feature = "bincode-serializer")]
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

/// JSON serializer using serde_json.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// 
/// Available when the `json` feature is enabled.
#[cfg(feature = "json")]
#[derive(Clone, Default)]
pub struct JsonSerializer;

#[cfg(feature = "json")]
impl JsonSerializer {
	/// Creates a new JSON serializer.
	pub fn new() -> Self {
		Self
	}
}

#[cfg(feature = "json")]
impl<T> MessageSerializer<T> for JsonSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = serde_json::Error;
	type DeserializeError = serde_json::Error;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		serde_json::to_vec(data)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		serde_json::from_slice(bytes)
	}
}

/// MessagePack serializer using rmp-serde.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// 
/// Available when the `messagepack` feature is enabled.
#[cfg(feature = "messagepack")]
#[derive(Clone, Default)]
pub struct MessagePackSerializer;

#[cfg(feature = "messagepack")]
impl MessagePackSerializer {
	/// Creates a new MessagePack serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "messagepack")]
impl<T> MessageSerializer<T> for MessagePackSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = rmp_serde::encode::Error;
	type DeserializeError = rmp_serde::decode::Error;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		rmp_serde::to_vec(data)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		rmp_serde::from_slice(bytes)
	}
}

/// CBOR serializer using ciborium.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// 
/// Available when the `cbor` feature is enabled.
#[cfg(feature = "cbor")]
#[derive(Clone, Default)]
pub struct CborSerializer;

#[cfg(feature = "cbor")]
impl CborSerializer {
	/// Creates a new CBOR serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "cbor")]
impl<T> MessageSerializer<T> for CborSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = ciborium::ser::Error<std::io::Error>;
	type DeserializeError = ciborium::de::Error<std::io::Error>;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		let mut buffer = Vec::new();
		ciborium::ser::into_writer(data, &mut buffer)?;
		Ok(buffer)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		ciborium::de::from_reader(bytes)
	}
}

/// Postcard serializer using postcard crate.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// Optimized for no_std environments and embedded systems.
/// 
/// Available when the `postcard` feature is enabled.
#[cfg(feature = "postcard")]
#[derive(Clone, Default)]
pub struct PostcardSerializer;

#[cfg(feature = "postcard")]
impl PostcardSerializer {
	/// Creates a new Postcard serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "postcard")]
impl<T> MessageSerializer<T> for PostcardSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = postcard::Error;
	type DeserializeError = postcard::Error;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		postcard::to_allocvec(data)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		postcard::from_bytes(bytes)
	}
}

/// Protocol Buffers serializer using prost crate.
///
/// Requires types to implement `prost::Message` trait.
/// Industry standard for high-performance data interchange.
/// 
/// Available when the `protobuf` feature is enabled.
#[cfg(feature = "protobuf")]
#[derive(Clone, Default)]
pub struct ProtobufSerializer;

#[cfg(feature = "protobuf")]
impl ProtobufSerializer {
	/// Creates a new Protobuf serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "protobuf")]
impl<T> MessageSerializer<T> for ProtobufSerializer
where T: prost::Message + Default + 'static
{
	type SerializeError = prost::EncodeError;
	type DeserializeError = prost::DecodeError;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		let mut buf = Vec::new();
		data.encode(&mut buf)?;
		Ok(buf)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		T::decode(bytes)
	}
}

/// RON (Rusty Object Notation) serializer using ron crate.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// Human-readable format ideal for configuration files and debugging.
/// 
/// Available when the `ron` feature is enabled.
#[cfg(feature = "ron")]
#[derive(Clone, Default)]
pub struct RonSerializer;

#[cfg(feature = "ron")]
impl RonSerializer {
	/// Creates a new RON serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "ron")]
impl<T> MessageSerializer<T> for RonSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = Box<dyn std::error::Error + Send + Sync>;
	type DeserializeError = Box<dyn std::error::Error + Send + Sync>;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		let string = ron::to_string(data)?;
		Ok(string.into_bytes())
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		let string = std::str::from_utf8(bytes)?; // Clean error handling without hacks
		Ok(ron::from_str(string)?)
	}
}

/// Flexbuffers serializer using flexbuffers crate.
///
/// Requires types to implement `serde::Serialize` and `serde::de::DeserializeOwned`.
/// Zero-copy schemaless binary format from Google FlatBuffers.
/// 
/// Available when the `flexbuffers` feature is enabled.
#[cfg(feature = "flexbuffers")]
#[derive(Clone, Default)]
pub struct FlexbuffersSerializer;

#[cfg(feature = "flexbuffers")]
impl FlexbuffersSerializer {
	/// Creates a new Flexbuffers serializer.
	pub fn new() -> Self {
		Self::default()
	}
}

#[cfg(feature = "flexbuffers")]
impl<T> MessageSerializer<T> for FlexbuffersSerializer
where T: Serialize + DeserializeOwned + 'static
{
	type SerializeError = flexbuffers::SerializationError;
	type DeserializeError = flexbuffers::DeserializationError;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError> {
		flexbuffers::to_vec(data)
	}

	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError> {
		flexbuffers::from_slice(bytes)
	}
}


