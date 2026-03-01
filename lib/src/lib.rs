use std::{
	marker::PhantomData,
	ops::{Deref, Range},
};

pub trait MatchAble {
	fn slice(&self, range: Range<usize>) -> &Self;
}
pub enum MatchSignal {
	Matched,
	MisMatched,
	InComplete,
	Other(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStatus {
	pub in_main_path: bool,
}
pub trait MatchBy<T> {
	fn match_by(&self, matcher: T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
pub trait Matcher<T: MatchAble + ?Sized> {
	fn try_match(self, matchable: &T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
pub struct MatchError {
	pub msg: String,
	pub ind: usize,
}
pub type MatchResult<T> = Result<T, MatchError>;
pub trait Matched<T: MatchAble> {
	fn matched(&self) -> &T;
}
impl<T> From<MatchResult<T>> for MatchSignal {
	fn from(value: MatchResult<T>) -> Self {
		match value {
			Ok(_) => MatchSignal::Matched,
			Err(err) => MatchSignal::Other(err.msg),
		}
	}
}
impl<T: MatchAble + ?Sized, U: Into<MatchSignal>, F: Fn(&T, &mut usize, &MatchStatus) -> U>
	MatchBy<&F> for T
{
	fn match_by(&self, matcher: &F, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		matcher(self, ind, status).into()
	}
}
