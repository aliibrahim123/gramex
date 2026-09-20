//! # gramex
//! grammar expressions, a common language for advance parsers.
//!
//! gramex is a [framework](Matcher) for building [ergonomic](matches!), [efficient](Mode) and [advance](cursor) parsers, [tokenizers](cursor::match_map) and any form of [grammar based transformers](crate::gramex).
//!
//! it simplifies parsing by providing simple [matching constructs](parse!) with [expressive DSL](gram_ref), while also featuring enriched [imperative experience](cursor) for advance use cases.
//!
//! # features
//! ## powerful core
//! gramex is universal in its core, everything can be [`MatchAble`], from [`str`](crate::str) to [`[u8]`](bytes) to [`YourTokenList`](MatchAble#impl-quide), provided the required infrastructure.
//!
//! it also adhere to rust zero cost abstraction principle, it leverage the power of [GATs](Mode) to enable its [`Matcher`]s get monomorphized into highly optimized code doing only the required [features](Mode#features).
//!
//! it also utilize zero copy parsing, it uses and produces [slices](MatchAble::slice) of the input by default to minimize allocations.
//!
//! ## simple DSL for simple cases
//! gramex feature its own custom [DSL](gram_ref), inspired by the typical metasyntax language, it has rich semantics. including and not limited to: [repetitions](gram_ref#repetitions), [negations](gram_ref#not-operator), [lookaheads](gram_ref#near-operator), [intersections](gram_ref#and-expression), [alternations](gram_ref#or-expression) and [implications](gram_ref#imply-expression).
//!
//! this grammar expressions support powerfull [capturing abilities](gram_ref#captures), with [nesting](gram_ref#structural-captures) and [enumeration](gram_ref#enumerated-captures) support, and [mapping](gram_ref#mapping) into [auto generated types](gram_ref#generated-items).
//!
//! this expressions can be used everywhere, declared inside [standalone definitions](crate::gramex), or used [inline](parse!) in the normal code, and even enriching the [imperative cursors](cursor::eat).
//!
//! ## imperative cursors for advance cases
//! gramex doesnt only generate simple parsers, it can empowers [advance parsers](cursor#why) through its imperative mode: the parsing [`cursor`]s.
//!
//! in the cursor mode, the parsing is done using unconstrained typical [imperative flow](cursor#example), enriched and integrated with the whole gramex ecosystem, through [declarative](cursor::match_map) and composable [utilities](cursor::Cursor).
//!
//! cursors are typically used with custom tokens lists, which can be [automaticly derived](derive::derive_slice_matchable) with their own [dedicated matchers](derive::derive_enum_matcher).
//!
//! # quick showcase
//! ```
//! fn basics() {
//!     // `matches` return `true` if a value matches a pattern
//!     // patterns are separated by whitespace, and can be literals, paths and blocks
//!     let c = "c";
//!     assert!(matches!("abc", 'a' "b" c));
//!
//!     // `~` matches without advancing, `!` matches one token if its pattern fails
//!     assert!(matches!("ac", ~"ac" !"b" 'c'));
//!
//!     // '?': optional, '+': 1..inf repetition, `[n]`: exact repetition
//!     assert!(matches!("bbccc", "a"? 'b'+ 'c'[3]));
//!
//!     // `..`: range matching, `_`: matches one token
//!     assert!(matches!("abc", 'a'..'z' _ ));
//!
//!     // `|`: match any pattern, `&`: match all patterns
//!     assert!(matches!("a", 'a' | 'b' | 'c'));
//!     assert!(matches!("b", 'a'..'z' & !'c'));
//!
//!     // captures: extract the matched section
//!     assert!(parse!("abc", 'a' bc:"bc").is_ok_and(|(bc,)| bc == "bc"));
//! }
//!
//! // grammar declaration, define multiple matchers
//! gramex! {
//!     for str;
//!     let ident: String = ('a'..'z' | 'A'..'Z' | '0'..'9' | '_')+;
//!     // `=> expr` mapping of the matched section (binded as `nb`)
//!     let nb: i64 = '-'? ('0'..'9')+ => nb.parse().unwrap();
//!     // `path<args>` compound matchers, `list<item, sep>`: list of `sep` seprated `item`s
//!     let arr: Vec<Val<'src>> = '[' list:list<val, ','> ']' => list;
//!     // generate an enum `Val`, can also use predefined matchers
//!     let val: enum = true:"true" | false:"false" | ident:ident | nb:nb | arr:arr;
//! }
//!
//! // (input, offset) tuple with some utilities, for ergonomic advance cases
//! type Cur<'src> = SimpleCursor<'src, str>;
//! fn parse_primary(cur: &mut Cur) -> Result<Expr, MatchError> {
//!     // `try_eat`: optional match by matcher, even from grammer declarations
//!     if let Some(num) = cur.try_eat(nb) {
//!         Ok(Expr::Num(num))
//!     } else if let Some(_ident) = cur.try_eat(ident) {
//!         Ok(Expr::Ident(_ident))
//!     } else {
//!         // become `expected one of identifier, number`
//!         cur.expected(["identifier", "number"])
//!     }
//! }
//! fn parse_expr(cur: &mut Cur) -> Result<Expr, MatchError> {
//!     // `eat`: match by a grammer expression
//!     let (name,) = eat!(cur, name:ident '=')?;
//!     let mut expr = parse_primary(cur)?;
//!     'op_loop: loop {
//!         // `match_map`: gramex `match` expression
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
//! gramex is minimal by default, and its featureset can be fainly selected by its feature flags.
//! - `std` (enabled by default): support for the standard library types.
//! - `macros`: enable all the macros.
//! - `str`: enable [`str`](crate::str) matching.
//! - `bytes`: enable [byte slices `[u8]`](crate::bytes) matching.
//! - `bits`: enable [`bits`](crate::bits) matching.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_bool)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::enum_glob_use)]

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(feature = "std")]
extern crate std as alloc_crate;

//#[cfg(doc)]
pub mod gram_ref;

mod core;
pub mod cursor;
pub mod general;
pub mod result;

pub use core::{MatchAble, Matcher, Mode, check, matches, parse, try_match};
pub mod modes {
	pub use crate::core::{Capture, Check, Parse, Test};
}

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

#[cfg(feature = "macros")]
pub use gramex_macro::{check, gramex, matcher, matches, parse, try_match};

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
