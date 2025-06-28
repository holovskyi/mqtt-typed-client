use std::marker::PhantomData;

use rumqttc::QoS;

use crate::{MqttClientError, TopicPatternPath};

/// Immutable builder for configuring MQTT publishers
#[derive(Debug, Clone)]
pub struct PublisherBuilder<MessageType> {
	qos: QoS,
	retain: bool,
	pattern: TopicPatternPath,
	_phantom: PhantomData<MessageType>,
}

impl<T> PublisherBuilder<T> {
	/// Create new builder with default pattern
	pub fn new(default_pattern: TopicPatternPath) -> Self {
		Self {
			qos: QoS::AtLeastOnce,
			retain: false,
			pattern: default_pattern,
			_phantom: PhantomData,
		}
	}

	/// Set QoS level
	pub fn with_qos(self, qos: QoS) -> Self {
		Self { qos, ..self }
	}

	/// Set retain flag
	pub fn with_retain(self, retain: bool) -> Self {
		Self { retain, ..self }
	}

	/// Replace pattern while preserving wildcard structure
	pub fn with_pattern(
		self,
		custom_pattern: impl TryInto<TopicPatternPath, Error: Into<MqttClientError>>,
	) -> Result<Self, MqttClientError> {
		let new_pattern = custom_pattern.try_into().map_err(Into::into)?;
		let validated_pattern = self.pattern.with_compatible_pattern(new_pattern)?;

		Ok(Self {
			pattern: validated_pattern,
			..self
		})
	}

	/// Get current QoS setting
	pub fn qos(&self) -> QoS {
		self.qos
	}

	/// Get current retain setting
	pub fn retain(&self) -> bool {
		self.retain
	}

	/// Get current pattern
	pub fn pattern(&self) -> &TopicPatternPath {
		&self.pattern
	}
}
