use crate::Mode;
use alloc_crate::{borrow::Cow, vec::Vec};
use core::fmt::{Debug, Display, Write};

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub enum Expected {
	#[default]
	None,
	A(Cow<'static, str>),
	OneOf(Vec<Cow<'static, str>>),
	Between(Cow<'static, str>, Cow<'static, str>),
}
impl Expected {
	pub fn value(&self) -> Cow<'static, str> {
		match self {
			Self::None => "".into(),
			Self::A(Cow::Borrowed(thing)) => Cow::Borrowed(thing),
			Self::A(Cow::Owned(thing)) => thing.clone().into(),
			Self::OneOf(things) => {
				let mut s = String::from("one of ");
				for (i, thing) in things.iter().enumerate() {
					write!(s, "{}{thing}", if i > 0 { ", " } else { "" }).unwrap();
				}
				s.into()
			}
			Self::Between(a, b) => format!("between {a} and {b}").into(),
		}
	}
}
impl Display for Expected {
	fn fmt(&self, f: &mut alloc_crate::fmt::Formatter<'_>) -> alloc_crate::fmt::Result {
		if !matches!(self, Self::None) {
			write!(f, "expected {}", self.value())
		} else {
			Ok(())
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
	fn fmt(&self, f: &mut alloc_crate::fmt::Formatter<'_>) -> alloc_crate::fmt::Result {
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
	fn fmt(&self, f: &mut alloc_crate::fmt::Formatter<'_>) -> alloc_crate::fmt::Result {
		write!(f, "{} at {}", self.kind, self.off)
	}
}
impl core::error::Error for MatchError where usize: Display + Debug {}

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
			true => M::ok_with(|| ()),
			false => M::err_with(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}
impl<T> IntoResult for Option<T> {
	type Output = T;
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<T, M> {
		match self {
			Some(v) => M::ok_with(|| v),
			None => M::err_with(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}
impl<T> IntoResult for Result<T, MatchError> {
	type Output = T;
	fn into_result<M: Mode>(self, _off: usize) -> MatchResult<T, M> {
		match self {
			Ok(v) => M::ok_with(|| v),
			Err(e) => M::err_with(|| e),
		}
	}
}
