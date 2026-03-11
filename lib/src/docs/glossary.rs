//! # the glossary
//! ### matchable
//! a type that can be matched by the library, it is a stream of tokens.
//!
//! it must implements the [`MatchAble`] trait
//!
//! ### matching
//! a type / value that a matchable can be matched against or by.
//!
//! the matchable must implement the [`MatchBy`] trait for the matching type.
//!
//! ### matcher
//! a function that matches a matchable, see the [`Matcher`] trait for more details.
//!
//! ### token
//! an individual unit of the matchable, like `char` for `str` and `u8` for `&[u8]`.
//!
//! ### parameterized matcher
//! a function that matches a matchable, taking a list of matchers as arguments.
//!
//! **signature**: `Fn (T, ...Matcher<T>, &mut usize, &MatchStatus) -> Into<MatchSignal> where T: MatchAble`
//!
//! these function can not be used as [`Matcher`], but with [`call` atoms](docs::gram_ref#call).
//!
//! ### capturing function
//! a function that matches like a matcher but returns a [`MatchResult`] of a capture instead of a [`MatchSignal`].
//!
//! **signature**: `Fn (T, &mut usize, &MatchStatus) -> MatchResult<Cap>` where `T` is the [`MatchAble`], and `Cap` is the capture type.
//!
//! these function can not be used as [`Matcher`], they are called with [term captures](docs::gram_ref#term).
//!
//! capturing functions can be parameterized like parameterized matchers, however must ba called rawly.
use crate::*;
