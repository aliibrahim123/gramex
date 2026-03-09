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
//! ### parenthesized matcher
//! a function that matches a matchable, taking a list of matchers as arguments.
//!
//! **signature**: `Fn (T, ...Matcher<T>, &mut usize, &MatchStatus) -> Into<MatchSignal> where T: MatchAble`
use crate::*;
