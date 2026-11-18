//! # gramex
//! grammar expressions: a common language for advanced parsers.
//!
//! gramex is a [framework](Matcher) for building [ergonomic](matches!), [efficient](Mode) and [advanced](cursor) parsers, [tokenizers](cursor::match_map) and any form of [grammar-based transformers](crate::grammar).
//!
//! it simplifies parsing by providing simple [matching constructs](parse!) with an [expressive DSL](gram_ref), while also featuring an enriched [imperative experience](cursor) for advanced use cases.
//!
//! # features
//! ## powerful core
//! gramex is universal at its core: anything can be [`MatchAble`], from [`str`](mod@str) to [`[u8]`](bytes) to [`YourTokenList`](MatchAble#impl-guide), provided the required infrastructure is implemented.
//!
//! it adheres to Rust's zero-cost abstraction principle, leveraging the power of [GATs](Mode) so that its [`Matcher`]s are monomorphized into highly optimized code that does only what is required ([features](Mode#features)).
//!
//! it also uses zero-copy parsing: by default, it consumes and produces [slices](MatchAble::slice) of the input to minimize allocations.
//!
//! ## simple DSL for simple cases
//! gramex features its own [DSL](gram_ref); inspired by typical metasyntax languages, it has rich semantics, including (but not limited to) [repetitions](gram_ref#repetitions), [negations](gram_ref#not-operator), [lookaheads](gram_ref#near-operator), [intersections](gram_ref#and-expression), [alternations](gram_ref#or-expression) and [implications](gram_ref#imply-expression).
//!
//! grammar expressions support powerful [capturing](gram_ref#captures), with [nesting](gram_ref#structural-captures), [enumeration](gram_ref#enumerated-captures) and [mapping](gram_ref#mapping) into [automatically generated types](gram_ref#generated-items).
//!
//! these expressions can be used everywhere: declared inside [standalone definitions](crate::grammar), used [inline](parse!) in normal code, and even enriching the [imperative cursors](cursor::eat).
//!
//! ## imperative cursors for advanced cases
//! gramex does not only generate simple parsers; it can also power advanced parsers through its imperative mode: the parsing [`cursor`]s.
//!
//! in cursor mode, parsing uses ordinary, unconstrained [imperative flow](cursor#example), enriched by and integrated with the whole gramex ecosystem through [declarative](cursor::match_map), composable [utilities](cursor::Cursor).
//!
//! cursors are typically used with custom token lists, which can be [derived automatically](derive::derive_slice_matchable), together with their own [dedicated matchers](derive::derive_enum_matcher).
//!
//! # quick showcase
//! ```
//! fn basics() {
//!     // `matches` returns `true` if a value matches a pattern
//!     // patterns are separated by whitespace, and can be literals, paths and blocks
//!     let c = "c";
//!     assert!(matches!("abc", 'a' "b" c));
//!
//!     // `~` matches without advancing, `!` matches one token if its pattern fails
//!     assert!(matches!("ac", ~"ac" !"b" 'c'));
//!
//!     // `?`: optional, `+`: one or more repetitions, `[n]`: exact repetition
//!     assert!(matches!("bbccc", "a"? 'b'+ 'c'[3]));
//!
//!     // `..`: range matching, `_`: matches one token
//!     assert!(matches!("abc", 'a'..'z' _ ));
//!
//!     // `|`: match any of the patterns, `&`: match all of the patterns
//!     assert!(matches!("a", 'a' | 'b' | 'c'));
//!     assert!(matches!("b", 'a'..'z' & !'c'));
//!
//!     // captures: extract the matched section
//!     assert!(parse!("abc", 'a' bc:"bc").is_ok_and(|(bc,)| bc == "bc"));
//! }
//!
//! // grammar declaration: defines multiple matchers
//! grammar! {
//!     for str;
//!     let ident: String = ('a'..'z' | 'A'..'Z' | '0'..'9' | '_')+;
//!     // `=> expr` maps the matched section (bound as `nb`)
//!     let nb: i64 = '-'? ('0'..'9')+ => nb.parse().unwrap();
//!     // `path<args>` compound matchers; `list<item, sep>`: a list of `item`s separated by `sep`
//!     let arr: Vec<Val<'src>> = '[' list:list<val, ','> ']' => list;
//!     // generates an enum `Val`; predefined matchers can be used too
//!     let val: enum = true:"true" | false:"false" | ident:ident | nb:nb | arr:arr;
//! }
//!
//! // an (input, offset) tuple with some utilities, for ergonomic handling of advanced cases
//! type Cur<'src> = SimpleCursor<'src, str>;
//! fn parse_primary(cur: &mut Cur) -> Result<Expr, MatchError> {
//!     // `try_eat`: optionally match a matcher, including ones from grammar declarations
//!     if let Some(num) = cur.try_eat(nb) {
//!         Ok(Expr::Num(num))
//!     } else if let Some(_ident) = cur.try_eat(ident) {
//!         Ok(Expr::Ident(_ident))
//!     } else {
//!         // produces `expected one of identifier, number`
//!         cur.expected(["identifier", "number"])
//!     }
//! }
//! fn parse_expr(cur: &mut Cur) -> Result<Expr, MatchError> {
//!     // `eat`: match with a grammar expression
//!     let (name,) = eat!(cur, name:ident '=')?;
//!     let mut expr = parse_primary(cur)?;
//!     'op_loop: loop {
//!         // `match_map`: gramex's `match` expression
//!         match_map!(cur, {
//!             '+' => expr = Expr::Add(Box::new(expr), Box::new(parse_primary(cur)?)),
//!             '-' => expr = Expr::Sub(Box::new(expr), Box::new(parse_primary(cur)?)),
//!             else => break 'op_loop,
//!         })
//!     }
//!     cur.eat(';')?;
//!     Ok(Expr::Let(name, Box::new(expr)))
//! }
//! ```
//!
//! # feature flags
//! gramex is minimal by default, and its features can be selected precisely through feature flags.
//! - `std` (enabled by default): support for standard library types.
//! - `macros`: enable all the macros.
//! - `str`: enable [`str`](mod@str) matching.
//! - `bytes`: enable [byte slice `[u8]`](crate::bytes) matching.
//! - `bits`: enable [`bits`] matching.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_bool)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::enum_glob_use)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(feature = "std")]
extern crate std as alloc_crate;

#[cfg(doc)]
pub mod gram_ref;

mod core;
#[cfg(feature = "macros")]
mod core_macros;
pub mod cursor;
pub mod general;
pub mod result;

pub use core::{MatchAble, Matcher, Mode, check, matches, parse, try_match};
/// the concrete [`Mode`] types
pub mod modes {
	pub use crate::core::{Capture, Check, Parse, Test};
}

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

#[cfg(feature = "macros")]
pub use core_macros::*;

#[cfg(feature = "bits")]
pub mod bits;
#[cfg(feature = "bytes")]
pub mod bytes;
#[cfg(feature = "str")]
pub mod str;

#[cfg(all(
	not(feature = "macros"),
	// they use it
	any(feature = "str", feature = "bytes", feature = "bits")
))]
pub(crate) mod derive;
#[cfg(feature = "macros")]
pub mod derive;
