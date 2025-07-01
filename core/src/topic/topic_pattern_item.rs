//! MQTT topic pattern item types and functionality

use std::borrow::Cow;
use std::convert::TryFrom;

use arcstr::Substr;
use thiserror::Error;

/// Error types for topic pattern parsing
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicPatternError {
	/// Hash wildcard (#) used not at the end of the pattern
	#[error(
		"Invalid topic pattern '{pattern}': # wildcard can only be the last \
		 segment"
	)]
	HashPosition {
		/// The invalid pattern
		pattern: String,
	},

	/// Wildcard characters (+ or #) used incorrectly
	#[error("Invalid wildcard usage: {usage}")]
	WildcardUsage {
		/// Description of invalid usage
		usage: String,
	},

	/// Empty topic is not valid
	#[error("Topic pattern cannot be empty")]
	EmptyTopic,

	/// Topic pattern structure mismatch when trying to use compatible pattern
	#[error(
		"Topic pattern structure mismatch.\nOriginal: '{original}'\nCustom:   \
		 '{custom}'\nHint: Both patterns must have the same parameter \
		 structure (same wildcards in same positions)"
	)]
	PatternStructureMismatch {
		/// Original pattern from the struct
		original: String,
		/// Custom pattern that doesn't match
		custom: String,
	},
}

impl TopicPatternError {
	/// Creates a new HashPosition error
	pub fn hash_position(pattern: impl Into<String>) -> Self {
		Self::HashPosition {
			pattern: pattern.into(),
		}
	}

	/// Creates a new WildcardUsage error
	pub fn wildcard_usage(usage: impl Into<String>) -> Self {
		Self::WildcardUsage {
			usage: usage.into(),
		}
	}

	/// Creates a new PatternStructureMismatch error
	pub fn pattern_mismatch(
		original: impl Into<String>,
		custom: impl Into<String>,
	) -> Self {
		Self::PatternStructureMismatch {
			original: original.into(),
			custom: custom.into(),
		}
	}
}

impl From<std::convert::Infallible> for TopicPatternError {
	fn from(_: std::convert::Infallible) -> Self {
		unreachable!("Infallible can never be constructed")
	}
}

/// MQTT topic pattern segment: literal string or wildcard
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopicPatternItem {
	/// Literal string segment
	Str(Substr),
	/// Single-level wildcard `+` or named `{param}`
	Plus(Option<Substr>),
	/// Multi-level wildcard `#` or named `{param:#}`
	Hash(Option<Substr>),
}

impl TopicPatternItem {
	/// Returns string representation of the pattern item.
	pub fn as_str(&self) -> &str {
		match self {
			| TopicPatternItem::Str(s) => s,
			| TopicPatternItem::Plus(_) => "+",
			| TopicPatternItem::Hash(_) => "#",
		}
	}

	/// Returns pattern representation with named parameters in braces.
	pub fn as_wildcard(&self) -> Cow<str> {
		match self {
			| TopicPatternItem::Plus(None) => Cow::Borrowed("+"),
			| TopicPatternItem::Hash(None) => Cow::Borrowed("#"),
			| TopicPatternItem::Plus(Some(name)) => {
				Cow::Owned(format!("{{{name}}}"))
			}
			| TopicPatternItem::Hash(Some(name)) => {
				Cow::Owned(format!("{{{name}:#}}"))
			}
			| TopicPatternItem::Str(s) => Cow::Borrowed(s),
		}
	}

	/// Returns parameter name for named wildcards.
	pub fn param_name(&self) -> Option<Substr> {
		match self {
			| TopicPatternItem::Plus(Some(name))
			| TopicPatternItem::Hash(Some(name)) => Some(name.clone()),
			| _ => None,
		}
	}

	/// Returns true if this item is a wildcard (+ or #).
	pub fn is_wildcard(&self) -> bool {
		matches!(self, TopicPatternItem::Plus(_) | TopicPatternItem::Hash(_))
	}
}

impl From<&TopicPatternItem> for String {
	fn from(item: &TopicPatternItem) -> Self {
		item.as_str().to_string()
	}
}

impl std::fmt::Display for TopicPatternItem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl TryFrom<Substr> for TopicPatternItem {
	type Error = TopicPatternError;
	fn try_from(item: Substr) -> Result<Self, Self::Error> {
		let res = match item.as_str() {
			| "+" => TopicPatternItem::Plus(None),
			| "#" => TopicPatternItem::Hash(None),
			| _ if item.starts_with("{") && item.ends_with(":#}") => {
				let inner =
					item.trim_start_matches('{').trim_end_matches(":#}");
				if inner.is_empty() {
					return Err(TopicPatternError::wildcard_usage(
						item.as_str(),
					));
				}
				TopicPatternItem::Hash(Some(item.substr_from(inner)))
			}
			| _ if item.starts_with("{") && item.ends_with("}") => {
				let inner = item.trim_start_matches('{').trim_end_matches("}");
				if inner.is_empty() {
					return Err(TopicPatternError::wildcard_usage(
						item.as_str(),
					));
				}
				TopicPatternItem::Plus(Some(item.substr_from(inner)))
			}
			| _ if item.contains(['+', '#']) => {
				return Err(TopicPatternError::wildcard_usage(item.as_str()));
			}
			| _ => TopicPatternItem::Str(item),
		};
		Ok(res)
	}
}
