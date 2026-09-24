//! items for a general [`MatchResult`] and its [`MatchError`].

use crate::Mode;
use alloc_crate::vec::Vec;
use core::{
	fmt::{self, Debug, Display, Formatter},
	ops::Range,
};
use lean_string::LeanString;

/// what was expected to be matched.
///
/// `Expected` is a data structure that express what the matching logic was expected to match but failed at a given point.
///
/// `Expected` is an optional part inside [`MatchError`] that is advised for every general matcher to provide for better error messages.
///
/// `Expected` uses [`LeanString`] to store small strings inline.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub enum Expected {
	/// undifined `Expected` value.
	///
	/// used when the `Expected` is difficult to compute or not important.
	///
	/// # example
	/// ```
	/// assert_eq!(Expected::None.to_string(), "");
	/// assert_eq!(Expected::default(), Expected::None);
	/// assert_eq!(<() as Matcher<str>>::expected(), Expected::None);
	/// ```
	#[default]
	None,

	/// expected whatever non empty thing.
	///
	/// usefull for [`skip_n`](crate::MatchAble::skip_n) and anything like it.
	///
	/// # example
	/// ```
	/// assert_eq!(Expected::SomeThing.to_string(), "something");
	/// assert_eq!(matcher!(for str, _).expected(), Expected::SomeThing);
	/// ```
	SomeThing,

	/// expected a specific thing.
	///
	/// the most common `Expected` kind, being olny any string.
	///
	/// # example
	/// ```
	/// assert_eq!(Expected::A("thing".into()).to_string(), "thing");
	/// assert_eq!(Expected::from("thing"), Expected::A("thing".into()));
	/// assert_eq!(matcher!(for str, "abc").expected(), Expected::A("\"abc\"".into()));
	/// ```
	A(LeanString),

	/// expected anything not a specific thing.
	///
	/// used by [not operator](crate::gram_ref#not-operator), and anything similar to it.
	///
	/// # example
	/// ```
	/// assert_eq!(Expected::Not("thing".into()).to_string(), "not thing");
	/// assert_eq!(matcher!(for str, !"a").expected(), Expected::Not("\"a\"".into()));
	/// ```
	Not(LeanString),

	/// expected one of specific things.
	///
	/// used by [or expression](crate::gram_ref#or-expression), and anything similar to it.
	///
	/// # example
	/// ```
	/// let expected = Expected::OneOf(vec!["thing1".into(), "thing2".into(), "thing3".into()]);
	/// assert_eq!(expected.to_string(), "one of thing1, thing2, thing3"#);
	/// assert_eq!(Expected::from(["thing1", "thing2", "thing3"]), expected);
	/// assert_eq!(
	///     matcher!(for str, "a" | "b" | "c").expected(),
	///     Expected::OneOf(vec!["\"a\"".into(), "\"b\"".into(), "\"c\"".into()]),
	/// );
	/// ```
	OneOf(Vec<LeanString>),

	/// expected a range of things.
	///
	/// used by [range atom](crate::gram_ref#range-atom) and range based [`Matcher`](crate::Matcher).
	///
	/// # example
	/// ```
	/// let expected = Expected::Between("thing1".into(), "thing2".into());
	/// assert_eq!(expected.to_string(), "between thing1 and thing2");
	/// assert_eq!(Expected::from("thing1".."thing2"), expected);
	/// assert_eq!(
	///     matcher!(for str, "a".."z").expected(),
	///     Expected::Between("\"a\"".into(), "\"z\"".into()),
	/// );
	/// ```
	Between(LeanString, LeanString),
}

impl From<&str> for Expected {
	fn from(value: &str) -> Self {
		Self::A(value.into())
	}
}
impl From<LeanString> for Expected {
	fn from(value: LeanString) -> Self {
		Self::A(value)
	}
}
impl<const N: usize> From<[&str; N]> for Expected {
	fn from(value: [&str; N]) -> Self {
		Self::OneOf(value.into_iter().map(Into::into).collect())
	}
}
impl<const N: usize> From<[LeanString; N]> for Expected {
	fn from(value: [LeanString; N]) -> Self {
		Self::OneOf(Vec::from(value))
	}
}
impl From<Range<&str>> for Expected {
	fn from(value: Range<&str>) -> Self {
		Self::Between(value.start.into(), value.end.into())
	}
}
impl From<Range<LeanString>> for Expected {
	fn from(value: Range<LeanString>) -> Self {
		Self::Between(value.start, value.end)
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

/// the what part of [`MatchError`]
///
/// the `MatchErrorKind` encode the kind of [`MatchError`], wheather it is [mismatch](Self::MisMatch), [incomplete](Self::InComplete), and whatever.
///
/// `MatchErrorKind` are generally not manually constructed, instead a contructor function is provided for every kind in [`MatchError`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchErrorKind {
	/// a mismatch between what was [`Expected`] and what was found.
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::mismatch(Expected::from("thing"), 1).kind,
	///     MatchErrorKind::MisMatch(Expected::A("thing".into())),
	/// );
	/// assert_eq!(check!("b", "a"), Err(MatchError::mismatch("\"a\"".into(), 0)));
	/// ```
	MisMatch(Expected),

	/// an incomplete input while [`Expected`] someething.
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::incomplete(Expected::from("thing"), 1).kind,
	///     MatchErrorKind::InComplete(Expected::A("thing".into())),
	/// );
	/// assert_eq!(check!("", "a"), Err(MatchError::incomplete("\"a\"".into(), 0)));
	/// ```
	InComplete(Expected),

	/// an excess input.
	///
	/// # example
	/// ```
	/// assert_eq!(MatchError::excess(1).kind, MatchErrorKind::Excess);
	/// assert_eq!(check!("ab", "a"), Err(MatchError::excess(1)));
	/// ```
	Excess,

	/// other kinds of user defined error.
	///
	/// just a string you define
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::other("custom error", 1).kind,
	///     MatchErrorKind::Other("custom error".into())
	/// );
	/// ```
	Other(LeanString),
}

/// an error encountered during matching.
///
/// the `MatchError` is a general and good enough error type used universally by every [`Matcher`](crate::Matcher) and the general utilities powered by it.
///
/// it is composed of a [`MatchErrorKind`] representing its kind, and `off`set representing where it occured, and optionally an [`Expected`] representing what was expected to be.
///
/// `MatchError` is generally constructed through the varoius constructors provided by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchError {
	/// the kind of the error
	pub kind: MatchErrorKind,
	/// the offset of where the error occured
	pub off: usize,
}
impl MatchError {
	/// create a `MatchError` of kind [`MisMatch`](MatchErrorKind::MisMatch).
	///
	/// it takes what is [`Expected`] and the offset where the error occured.
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::mismatch("thing".into(), 1).to_string(),
	///     "mismatch at 1, expected thing",
	/// );
	/// assert_eq!(check!("b", "a"), Err(MatchError::mismatch("\"a\"".into(), 0)));
	/// ```
	pub fn mismatch(expected: Expected, off: usize) -> Self {
		Self { kind: MatchErrorKind::MisMatch(expected), off }
	}

	/// create a `MatchError` of kind [`InComplete`](MatchErrorKind::InComplete).
	///
	/// it takes what is [`Expected`] and the offset where the error occured.
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::incomplete("thing".into(), 1).to_string(),
	///     "incomplete input at 1, expected thing",
	/// );
	/// assert_eq!(check!("", "a"), Err(MatchError::incomplete("\"a\"".into(), 0)));
	/// ```
	pub fn incomplete(expected: Expected, off: usize) -> Self {
		Self { kind: MatchErrorKind::InComplete(expected), off }
	}

	/// create a `MatchError` expecting an [`Expected`].
	///
	/// the result error kind is either [`MisMatch`](MatchErrorKind::MisMatch) or [`InComplete`](MatchErrorKind::InComplete) depending on `is_incomplete`.
	///
	/// it take an offset where the error occured.
	///
	/// # example
	/// ```
	/// assert_eq!(
	///     MatchError::expected("thing".into(), false, 1),
	///     MatchError::mismatch("thing".into(), 1),
	/// );
	/// assert_eq!(
	///     MatchError::expected("thing".into(), true, 1),
	///     MatchError::incomplete("thing".into(), 1),
	/// );
	///
	/// // in matcher
	/// M::err(|| MatchError::expected("thing".into(), *off == matched.len(), *off))
	/// ```
	pub fn expected(expected: Expected, is_incomplete: bool, off: usize) -> Self {
		match is_incomplete {
			true => Self::incomplete(expected, off),
			false => Self::mismatch(expected, off),
		}
	}

	/// create a `MatchError` of kind [`Excess`](MatchErrorKind::Excess).
	///
	/// it takes the offset where the error occured.
	///
	/// # example
	/// ```
	/// assert_eq!(MatchError::excess(1).to_string(), "excess input at 1");
	/// assert_eq!(check!("ab", "a"), Err(MatchError::excess(1)));
	/// ```
	pub fn excess(off: usize) -> Self {
		Self { kind: MatchErrorKind::Excess, off }
	}

	/// create a `MatchError` of kind [`Other`](MatchErrorKind::Other).
	///
	/// it takes a message and the offset where the error occured.
	///
	/// # example
	/// ```
	/// assert_eq!(MatchError::other("custom error", 1).to_string(), "custom error");
	/// ```
	pub fn other(msg: impl Into<LeanString>, off: usize) -> Self {
		Self { kind: MatchErrorKind::Other(msg.into()), off }
	}
}

impl Display for MatchError {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		use MatchErrorKind::*;
		let Self { kind, off } = self;
		match kind {
			MisMatch(_) => write!(f, "mismatch at {off}")?,
			InComplete(_) => write!(f, "incomplete input at {off}")?,
			Excess => write!(f, "excess input at {off}")?,
			Other(msg) => f.write_str(msg)?,
		}
		if let MisMatch(expected) | InComplete(expected) = kind {
			write!(f, ", expected {expected}")?;
		}
		Ok(())
	}
}
impl core::error::Error for MatchError {}

/// the matching result created universally by every [`Matcher`](crate::Matcher).
///
/// `MatchResult` is a `Result<T, MatchError>` generic over [`Mode`], it is:
///
/// | [`Mode`]                           | `Ok` | `Err`          | simplified form |
/// | ---------------------------------- | ---- |- ------------- | --------------- |
/// | [`Test`](crate::modes::Test)       | `()` | `()`           | `bool`          |
/// | [`Check`](crate::modes::Check)     | `()` | [`MatchError`] | same            |
/// | [`Capture`](crate::modes::Capture) | `T`  | `()`           | `Option<T>`     |
/// | [`Parse`](crate::modes::Parse)     | `T`  | [`MatchError`] | same            |
#[allow(type_alias_bounds)]
pub type MatchResult<T, M: Mode> = Result<M::Success<T>, M::Error>;

/// convert the type into a [`MatchResult`].
///
/// [`into_result`](Self::into_result) take an offset for `Err` if needed and the [`Mode`] and return a [`MatchResult`] corresponding to `self`.
///
/// the implementing types are:
///
/// | type             | `Ok`          | `Err`                                                          |
/// | ---------------- | ------------- | -------------------------------------------------------------- |
/// | [`bool`]         | `()`          | [`MatchError::mismatch(Expected::None)`](MatchError::mismatch) |
/// | [`Option<T>`]    | `T` in `Some` | [`MatchError::mismatch(Expected::None)`](MatchError::mismatch) |
/// | [`Result<T, MatchError>`] | `T` in `Ok` | [`MatchError`] in `Err`                                 |
///
/// # example
/// ```
/// assert_eq!(true.into_result::<Parse>(0), Ok(()));
/// assert_eq!(false.into_result::<Parse>(0), Err(MatchError::mismatch(Expected::None, 0)));
///
/// assert_eq!(Some(1).into_result::<Parse>(1), Ok(1));
/// assert_eq!(None::<u8>.into_result::<Parse>(1), Err(MatchError::mismatch(Expected::None, 1)));
///
/// assert_eq!(Ok(1).into_result::<Parse>(1), Ok(1));
/// assert_eq!(
///     Err::<u8, _>(MatchError::mismatch("thing".into(), 1)).into_result::<Parse>(1),
///     Err(MatchError::mismatch("thing".into(), 1))
/// );
/// ```
pub trait IntoResult: Sized {
	/// the `Ok` type of [`MatchResult`], not modified by the [`Mode`].
	type Success;

	/// convert the type into a [`MatchResult`], given an offset and a [`Mode`].
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<Self::Success, M>;
}
impl IntoResult for bool {
	type Success = ();
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<(), M> {
		match self {
			true => M::ok(|| ()),
			false => M::err(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}
impl<T> IntoResult for Option<T> {
	type Success = T;
	fn into_result<M: Mode>(self, off: usize) -> MatchResult<T, M> {
		match self {
			Some(v) => M::ok(|| v),
			None => M::err(|| MatchError::mismatch(Expected::None, off)),
		}
	}
}

impl<T> IntoResult for Result<T, MatchError> {
	type Success = T;
	fn into_result<M: Mode>(self, _off: usize) -> MatchResult<T, M> {
		match self {
			Ok(v) => M::ok(|| v),
			Err(e) => M::err(|| e),
		}
	}
}
