use std::{
	borrow::Cow,
	fmt::{Debug, Display},
};

use crate::{MatchAble, Mode};

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub enum Expected {
	#[default]
	None,
	A(Cow<'static, str>),
	OneOf(Vec<Cow<'static, str>>),
	Between(Cow<'static, str>, Cow<'static, str>),
}
impl Display for Expected {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::None => write!(f, ""),
			Self::A(thing) => write!(f, "expected {thing}"),
			Self::OneOf(things) => {
				write!(f, "expected one of ")?;
				for (i, thing) in things.iter().enumerate() {
					write!(f, "{}{thing}", if i > 0 { ", " } else { "" })?
				}
				Ok(())
			}
			Self::Between(a, b) => write!(f, "expected between {a} and {b}"),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchErrorKind {
	MisMatch(Expected),
	InComplete(Expected),
	Excess,
	Other(Cow<'static, str>),
}
impl Display for MatchErrorKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::MisMatch(expected) => match expected {
				Expected::None => write!(f, "mismatch"),
				_ => write!(f, "mismatch, {expected}"),
			},
			Self::InComplete(expected) => match expected {
				Expected::None => write!(f, "incomplete input"),
				_ => write!(f, "incomplete input, {expected}"),
			},
			MatchErrorKind::Excess => write!(f, "excess input"),
			MatchErrorKind::Other(msg) => write!(f, "{msg}"),
		}
	}
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
	pub fn other(msg: impl Into<Cow<'static, str>>, off: usize) -> Self {
		Self { kind: MatchErrorKind::Other(msg.into()), off }
	}
}

impl Display for MatchError
where
	usize: Display,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} at {}", self.kind, self.off)
	}
}
impl std::error::Error for MatchError where usize: Display + Debug {}

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
