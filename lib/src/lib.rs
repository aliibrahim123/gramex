//! # gramex
//! grammer expressions, a common language for advance parsers.
//!
//! gramex is a library and a simple language for building parsers, tokenizers and other forms of grammer based transformers.
//!
//! it simplify parsing by transforming a simple yet expressive grammer declerations into efficient reusable matcher functions.
//!
//! # features
//! - **type agnostic matching:** parse `str`, byte slices (`[u8]`), or custom token streams.
//! - **zero cost abstractions:** grammers compile down to highly optimized, near metal matcher functions.
//! - **rich grammar syntax:** native support for repetitions, alternations (`|`), intersections (`&`), ranges (`..`), lookahead peeks (`~`), and negations (`!`).
//! - **powerful capturing & mapping:** extract sections, nested or enumerated, and map them into custom types.
//! - **extensible throught code**: just drop your custom matcher inside `{}` block.
//! - term based grammer defenition thought [`gramex`], or inlined expression matching through [`matches`] and [`try_match`]
//! - **batteries included**: comes with various built-in helpers and standard patterns.
//!
//! # quick guide
//! ```
//! // quick matching can be done using `matches` macro
//! // matches agianst items by literals, path or blocks
//! assert!(matches!("abc": str, "abc"));
//! let pat = "abc";
//! assert!(matches!("abc": str, pat));
//! assert!(matches!("bc": str, { &pat[1..] }));
//!
//! // patterns are separated by whitespace
//! assert!(matches!("abc": str, 'a' 'b' 'c'));
//!
//! // `?`: optional, `*`: +0 repetition, `+`: +1 repetition
//! // `[count]`: exact repetition, `[min..max]`: ranged repetition
//! assert!(matches!("abbccc": str, 'a'? 'b'+ 'c'[3]));
//!
//! // `!`: matches one item if pattern doesnt match
//! // `~`: matches a pattern without advancing
//! assert!(matches!("cba": str, !'a' ~'b' "ba"));
//!
//! // `_`: matches any, `..` range match
//! assert!(matches!("abc": str, 'a'..'z' _ 'c'));
//!
//! // `|`: match any of the pattern
//! // `&`: match if all patterns matches
//! assert!(matches!("b": str, 'a' | 'b' | 'c'));
//! assert!(matches!("b": str, 'a'..'z' & !'c'));
//!
//! // capture are done using `(name = pattern)`
//! assert!(try_match!("abc": str, 'a' (bc = "bc")).is_ok_and(|v| v.bc == "bc"));
//! ```
//!
//! # other documentations
//! - [grammer reference](`docs::gram_ref`): documenting the grammer language syntax and its behaviours.
//!
use std::ops::Range;

mod str;
#[cfg(doc)]
pub mod docs {
	pub mod glossary;
	pub mod gram_ref;
}

pub use gramex_macro::{gramex, matcher, matches, try_match};

pub trait MatchAble {
	fn len(&self) -> usize;
	fn slice(&self, range: Range<usize>) -> &Self;
	fn skip_1(&self, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
pub trait MatchBy<T> {
	fn match_by(&self, matcher: T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
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
		!core::matches!(self, Self::Matched)
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
impl<T> From<MatchResult<T>> for MatchSignal {
	fn from(value: MatchResult<T>) -> Self {
		match value {
			Ok(_) => MatchSignal::Matched,
			Err(err) => MatchSignal::Other(err.msg),
		}
	}
}

pub trait Matcher<T: MatchAble + ?Sized>: for<'a> MatcherBridge<'a, T> {}
impl<T: MatchAble + ?Sized, F: for<'a> MatcherBridge<'a, T>> Matcher<T> for F {}
#[doc(hidden)]
pub trait MatcherBridge<'a, T: ?Sized> {
	fn call(&self, value: &'a T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
impl<T: MatchAble + ?Sized, F: for<'a> MatcherBridge<'a, T>> MatchBy<F> for T {
	fn match_by(&self, matcher: F, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		matcher.call(self, ind, status)
	}
}
impl<'a, T: MatchAble + ?Sized, F, R: Into<MatchSignal>> MatcherBridge<'a, T> for F
where
	F: Fn(&'a T, &mut usize, &MatchStatus) -> R,
{
	fn call(&self, val: &'a T, i: &mut usize, s: &MatchStatus) -> MatchSignal {
		self(val, i, s).into()
	}
}

pub fn by<T: MatchAble + ?Sized, F: for<'a> Fn(&'a T, &mut usize, &MatchStatus) -> MatchSignal>(
	matcher: F,
) -> F {
	matcher
}
#[doc(hidden)]
pub mod __private {
	pub fn conv<T, U>(cap: T, conv: impl Fn(T) -> U) -> U {
		conv(cap)
	}
	pub fn _as<T>(v: T) -> T {
		v
	}
}
