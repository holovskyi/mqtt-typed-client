use std::marker::PhantomData;

use rumqttc::QoS;

use crate::{MqttClientError, TopicPatternPath};

/// Immutable builder for configuring MQTT publishers
#[derive(Debug, Clone)]
pub struct PublisherBuilder<MessageType> {
	qos: QoS,
	retain: bool,
	pattern: TopicPatternPath,
	/// Flag indicating if the default pattern was overridden with a custom one.
	/// Used for performance optimization - when false, we can use compile-time formatting.
	pattern_overridden: bool,
	_phantom: PhantomData<MessageType>,
}

impl<T> PublisherBuilder<T> {
	/// Create new builder with default pattern
	pub fn new(default_pattern: TopicPatternPath) -> Self {
		Self {
			qos: QoS::AtLeastOnce,
			retain: false,
			pattern: default_pattern,
			pattern_overridden: false,
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
		let validated_pattern = self.pattern.check_pattern_compatibility(new_pattern)?;

		Ok(Self {
			pattern: validated_pattern,
			pattern_overridden: true,
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

	/// Check if pattern was overridden
	pub fn is_pattern_overridden(&self) -> bool {
		self.pattern_overridden
	}
}
