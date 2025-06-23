#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]

use std::fmt;
use std::sync::Arc;
use std::{collections::HashMap, ops::Range};

use arcstr::{ArcStr, Substr};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct TopicPath {
	pub path: ArcStr,
	pub segments: Vec<Substr>,
}

impl TopicPath {
	pub fn new(path: ArcStr) -> Self {
		let segments: Vec<Substr> =
			path.split('/').map(|s| path.substr_from(s)).collect();
		Self { path, segments }
	}

	pub fn path(&self) -> ArcStr {
		self.path.clone()
	}
}

impl fmt::Display for TopicPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.path)
	}
}

#[derive(Debug)]
pub enum TopicMatchError {
	UnexpectedEndOfPattern,
	UnexpectedEndOfTopic,
	UnexpectedHashSegment,
	SegmentMismatch {
		expected: String,
		found: String,
		position: usize,
	},
	DuplicateParameterName,
}

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

	pub fn get_param(&self, index: usize) -> Option<Substr> {
		self.params
			.get(index)
			.map(|range| self.get_param_range(range))
	}

	pub fn get_named_param(&self, name: &str) -> Option<Substr> {
		self.named_params
			.iter()
			.find(|(n, _)| n.as_str() == name)
			.map(|(_, range)| self.get_param_range(range))

		//self.named_params.get(name).map(|range| self.get_param_range(range))
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
