use std::fmt::Debug;

use bincode::{Decode, Encode};

pub trait MessageSerializer<T>: Default + Clone + Send + Sync + 'static {
	type SerializeError: Debug + Send + Sync + 'static;
	type DeserializeError: Debug + Send + Sync + 'static;

	fn serialize(&self, data: &T) -> Result<Vec<u8>, Self::SerializeError>;
	fn deserialize(&self, bytes: &[u8]) -> Result<T, Self::DeserializeError>;
}

#[derive(Clone, Default)]
pub struct BincodeSerializer {
	config: bincode::config::Configuration,
}

impl BincodeSerializer {
	pub fn new() -> Self {
		Self::default()
	}

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
