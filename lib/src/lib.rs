use std::ops::Range;

mod str;

pub use gramex_macro::gramex;

pub trait MatchAble {
	fn len(&self) -> usize;
	fn slice(&self, range: Range<usize>) -> &Self;
	fn skip_1(&self, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum MatchSignal {
	#[default]
	Matched,
	MisMatched,
	InComplete,
	Excess,
	Other(String),
}
impl MatchSignal {
	pub fn into_err(self, ind: usize) -> MatchError {
		match self {
			Self::Matched => MatchError::other(format!("being normal at {ind}"), ind),
			Self::MisMatched => MatchError::mismatch(ind),
			Self::InComplete => MatchError::incomplete(ind),
			Self::Excess => MatchError::excess(ind),
			Self::Other(msg) => MatchError::other(msg, ind),
		}
	}
	pub fn is_err(&self) -> bool {
		!matches!(self, Self::Matched)
	}
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchStatus {
	pub in_main_path: bool,
}
impl Default for MatchStatus {
	fn default() -> Self {
		Self { in_main_path: true }
	}
}
pub trait MatchBy<T> {
	fn match_by(&self, matcher: T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
pub trait Matcher<T: MatchAble + ?Sized>: for<'a> MatcherBridge<'a, T> {}
impl<T: MatchAble + ?Sized, F: for<'a> MatcherBridge<'a, T>> Matcher<T> for F {}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchError {
	pub msg: String,
	pub ind: usize,
}
impl MatchError {
	pub fn other(msg: String, ind: usize) -> Self {
		Self { msg, ind }
	}
	pub fn mismatch(ind: usize) -> Self {
		Self { msg: format!("mismatch at {ind}"), ind }
	}
	pub fn incomplete(ind: usize) -> Self {
		Self { msg: format!("incomplete input at {ind}"), ind }
	}
	pub fn excess(ind: usize) -> Self {
		Self { msg: format!("excess input at {ind}"), ind }
	}
}
impl std::fmt::Display for MatchError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.msg)
	}
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

impl<T: MatchAble + ?Sized, F: for<'a> MatcherBridge<'a, T>> MatchBy<F> for T {
	fn match_by(&self, matcher: F, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		matcher.call(self, ind, status)
	}
}
pub trait MatcherBridge<'a, T: ?Sized> {
	fn call(&self, value: &'a T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
impl<'a, T: MatchAble + ?Sized, F, R: Into<MatchSignal>> MatcherBridge<'a, T> for F
where
	F: Fn(&'a T, &mut usize, &MatchStatus) -> R,
{
	fn call(&self, val: &'a T, i: &mut usize, s: &MatchStatus) -> MatchSignal {
		self(val, i, s).into()
	}
}
pub fn by<T: MatchAble + ?Sized, F: Fn(&T, &mut usize, &MatchStatus) -> MatchSignal>(
	matcher: F,
) -> F {
	matcher
}
#[doc(hidden)]
pub mod __private {
	pub fn conv<T, U>(cap: T, conv: impl Fn(T) -> U) -> U {
		conv(cap)
	}
}
