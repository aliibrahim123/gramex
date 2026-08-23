use std::{
	borrow::Cow,
	fmt::{Debug, Display},
};

use crate::MatchAble;

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

pub struct MatchError<M: MatchAble + ?Sized> {
	pub kind: MatchErrorKind,
	pub off: M::Offset,
}
impl<M: MatchAble + ?Sized> MatchError<M> {
	pub fn mismatch(expected: Expected, off: M::Offset) -> Self {
		Self { kind: MatchErrorKind::MisMatch(expected), off }
	}
	pub fn incomplete(expected: Expected, off: M::Offset) -> Self {
		Self { kind: MatchErrorKind::InComplete(expected), off }
	}
	pub fn excess(off: M::Offset) -> Self {
		Self { kind: MatchErrorKind::Excess, off }
	}
	pub fn other(msg: impl Into<Cow<'static, str>>, off: M::Offset) -> Self {
		Self { kind: MatchErrorKind::Other(msg.into()), off }
	}
}

impl<M: MatchAble + ?Sized> Debug for MatchError<M>
where
	M::Offset: Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MatchError")
			.field("kind", &self.kind)
			.field("off", &self.off)
			.finish()
	}
}
impl<M: MatchAble + ?Sized> Clone for MatchError<M> {
	fn clone(&self) -> Self {
		Self { kind: self.kind.clone(), off: self.off }
	}
}
impl<M: MatchAble + ?Sized> PartialEq for MatchError<M> {
	fn eq(&self, other: &Self) -> bool {
		self.kind == other.kind && self.off == other.off
	}
}
impl<M: MatchAble + ?Sized> Display for MatchError<M>
where
	M::Offset: Display,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} at {}", self.kind, self.off)
	}
}
impl<M: MatchAble + ?Sized> std::error::Error for MatchError<M> where M::Offset: Display + Debug {}

#[allow(type_alias_bounds)]
pub type MatchResult<T, M: MatchAble> = Result<T, MatchError<M>>;
