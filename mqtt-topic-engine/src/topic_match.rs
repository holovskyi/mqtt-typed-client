//! Matched-topic types.
//!
//! [`TopicPath`] is a concrete topic split into segments; [`TopicMatch`] is the
//! result of matching such a topic against a pattern, exposing the captured
//! positional and named parameters.

#![allow(clippy::missing_docs_in_private_items)]

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use arcstr::{ArcStr, Substr};
use smallvec::SmallVec;
use thiserror::Error;

/// A concrete MQTT topic, split into its `/`-delimited segments.
///
/// The original topic string and the segment slices share the same backing
/// [`ArcStr`] allocation, so cloning and slicing are cheap.
#[derive(Debug, Clone)]
pub struct TopicPath {
	/// The full topic string.
	pub path: ArcStr,
	/// The topic split on `/`; each segment is a slice into [`path`](Self::path).
	pub segments: Vec<Substr>,
}

impl TopicPath {
	/// Builds a [`TopicPath`] by splitting `path` on `/` into segments.
	pub fn new(path: impl Into<ArcStr>) -> Self {
		let path = path.into();
		let segments: Vec<Substr> =
			path.split('/').map(|s| path.substr_from(s)).collect();
		Self { path, segments }
	}

	/// Returns a cheap (refcounted) clone of the full topic string.
	pub fn path(&self) -> ArcStr {
		self.path.clone()
	}
}

impl fmt::Display for TopicPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.path)
	}
}

/// Errors returned when matching a topic against a pattern.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TopicMatchError {
	/// Pattern ended unexpectedly while matching topic
	#[error("Pattern ended unexpectedly while matching topic")]
	UnexpectedEndOfPattern,

	/// Topic ended unexpectedly while matching pattern
	#[error("Topic ended unexpectedly while matching pattern")]
	UnexpectedEndOfTopic,

	/// Hash wildcard (#) found in unexpected position
	#[error("Hash wildcard (#) found in unexpected position")]
	UnexpectedHashSegment,

	/// Segment mismatch during topic matching
	#[error(
		"Segment mismatch at position {position}: expected '{expected}', \
		 found '{found}'"
	)]
	SegmentMismatch {
		/// Expected segment value
		expected: String,
		/// Actually found segment value
		found: String,
		/// Position where mismatch occurred
		position: usize,
	},

	/// Duplicate parameter name found in pattern
	#[error("Duplicate parameter name found in pattern")]
	DuplicateParameterName,
}

/// The result of matching a [`TopicPath`] against a pattern.
///
/// Holds the matched topic plus the ranges of segments captured by the
/// pattern's wildcards, accessible by position ([`get_param`](Self::get_param))
/// or by name ([`get_named_param`](Self::get_named_param)).
pub struct TopicMatch {
	topic: Arc<TopicPath>,
	params: SmallVec<[Range<usize>; 3]>,
	named_params: SmallVec<[(Substr, Range<usize>); 3]>,
}

impl TopicMatch {
	pub(crate) fn from_match_result(
		topic: Arc<TopicPath>,
		params: SmallVec<[Range<usize>; 3]>,
		named_params: SmallVec<[(Substr, Range<usize>); 3]>,
	) -> Self {
		Self {
			topic,
			params,
			named_params,
		}
	}

	/// Returns the matched topic's segments.
	pub fn path_segments(&self) -> &Vec<Substr> {
		&self.topic.segments
	}

	fn get_param_range(&self, range: &Range<usize>) -> Substr {
		if range.is_empty() {
			self.topic.path.substr(0 .. 0)
		} else if range.len() == 1 {
			self.topic.segments[range.start].clone()
		} else {
			let start_segment = &self.topic.segments[range.start];
			let end_segment = &self.topic.segments[range.end - 1];

			let start_pos = start_segment.as_ptr() as usize
				- self.topic.path.as_ptr() as usize;
			let end_pos = end_segment.as_ptr() as usize
				- self.topic.path.as_ptr() as usize
				+ end_segment.len();

			self.topic.path.substr(start_pos .. end_pos)
		}
	}

	/// Returns the positional parameter captured at `index`, if any.
	///
	/// Parameters are numbered in pattern order; a `#` wildcard yields the
	/// joined remainder of the topic.
	pub fn get_param(&self, index: usize) -> Option<Substr> {
		self.params
			.get(index)
			.map(|range| self.get_param_range(range))
	}

	/// Returns the value of the named parameter `name`, if the pattern bound one.
	pub fn get_named_param(&self, name: &str) -> Option<Substr> {
		self.named_params
			.iter()
			.find(|(n, _)| n.as_str() == name)
			.map(|(_, range)| self.get_param_range(range))

		//self.named_params.get(name).map(|range| self.get_param_range(range))
	}

	/// Returns a cheap (refcounted) clone of the matched topic string.
	pub fn topic_path(&self) -> ArcStr {
		self.topic.path.clone()
	}
}

//Implement Debug for TopicMatch, using get_param and get_named_param
impl fmt::Debug for TopicMatch {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "TopicMatch {{ topic: {}, params: [", self.topic.path)?;
		for (i, param) in self.params.iter().enumerate() {
			if i > 0 {
				write!(f, ", ")?;
			}
			write!(f, "{}", self.get_param_range(param))?;
		}
		write!(f, "]")?;

		if !self.named_params.is_empty() {
			write!(f, ", named_params: {{")?;
			for (name, range) in &self.named_params {
				write!(f, "{}: {}, ", name, self.get_param_range(range))?;
			}
			write!(f, "}}")?;
		}

		write!(f, " }}")
	}
}

impl fmt::Display for TopicMatch {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Match({})", self.topic.path)?;

		if !self.params.is_empty() {
			write!(f, " with {} params", self.params.len())?;
		}

		Ok(())
	}
}
