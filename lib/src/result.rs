use crate::Mode;
use alloc_crate::{borrow::Cow, vec::Vec};
use core::{
	fmt::{self, Debug, Display, Formatter, Write},
	ops::Range,
};
use lean_string::LeanString;

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub enum Expected {
	#[default]
	None,
	SomeThing,
	A(LeanString),
	Not(LeanString),
	OneOf(Vec<LeanString>),
	Between(LeanString, LeanString),
}
impl From<&str> for Expected {
	fn from(value: &str) -> Self {
		Self::A(value.into())
	}
}
impl<const N: usize> From<[&str; N]> for Expected {
	fn from(value: [&str; N]) -> Self {
		Self::OneOf(value.into_iter().map(|s| s.into()).collect())
	}
}
impl From<Range<&str>> for Expected {
	fn from(value: Range<&str>) -> Self {
		Self::Between(value.start.into(), value.end.into())
	}
}
impl Display for Expected {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match self {
			Self::None => Ok(()),
			Self::SomeThing => f.write_str("something"),
			Self::A(thing) => f.write_str(thing),
			Self::Not(thing) => write!(f, "not {thing}"),
			Self::OneOf(things) => {
				f.write_str("one of ")?;
				for (i, thing) in things.iter().enumerate() {
					write!(f, "{}{thing}", if i > 0 { ", " } else { "" })?;
				}
				Ok(())
			}
			Self::Between(a, b) => write!(f, "between {a} and {b}"),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchErrorKind {
	MisMatch(Expected),
	InComplete(Expected),
	Excess,
	Other(LeanString),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchError {
	pub kind: MatchErrorKind,
	pub off: usize,
}
impl MatchError {
	pub fn mismatch(expected: Expected, off: usize) -> Self {
		Self { kind: MatchErrorKind::MisMatch(expected), off }
	}
	pub fn incomplete(expected: Expected, off: usize) -> Self {
		Self { kind: MatchErrorKind::InComplete(expected), off }
	}
	pub fn excess(off: usize) -> Self {
		Self { kind: MatchErrorKind::Excess, off }
	}
	pub fn other(msg: impl Into<LeanString>, off: usize) -> Self {
		Self { kind: MatchErrorKind::Other(msg.into()), off }
	}
}

impl Display for MatchError {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let Self { kind, off } = self;
		let (kind, expected) = match kind {
			MatchErrorKind::MisMatch(expected) => ("mismatch", expected),
			MatchErrorKind::InComplete(expected) => ("incomplete", expected),
			MatchErrorKind::Excess => ("excess input", &Expected::None),
			MatchErrorKind::Other(msg) => (msg.as_ref(), &Expected::None),
		};
		write!(f, "{kind} at {off}")?;
		if *expected != Expected::None {
			write!(f, ", expected {expected}")?;
		}
		Ok(())
	}
}
impl core::error::Error for MatchError {}

#[allow(type_alias_bounds)]
pub type MatchResult<T, M: Mode> = Result<M::Success<T>, M::Error>;

pub trait IntoResult: Sized {
	type Output;
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<Self::Output, M>;
}
impl IntoResult for bool {
	type Output = ();
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<(), M> {
		match self {
			true => M::ok(|| ()),
			false => M::err(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}
impl<T> IntoResult for Option<T> {
	type Output = T;
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<T, M> {
		match self {
			Some(v) => M::ok(|| v),
			None => M::err(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}

impl<T> IntoResult for Result<T, MatchError> {
	type Output = T;
	fn into_result<M: Mode>(self, _off: usize) -> MatchResult<T, M> {
		match self {
			Ok(v) => M::ok(|| v),
			Err(e) => M::err(|| e),
		}
	}
}
