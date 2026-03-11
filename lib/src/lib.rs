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
//! - **rich grammar syntax:** native support for [`repetitions`](docs::gram_ref#repetition), alternations ([`|`](docs::gram_ref#or)), intersections ([`&`](docs::gram_ref#and)), ranges ([`..`](docs::gram_ref#range)), lookahead peeks ([`~`](docs::gram_ref#near-)), and negations ([`!`](docs::gram_ref#not-)).
//! - **powerful capturing & mapping:** extract sections, nested or enumerated, and map them into custom types.
//! - **extensible throught code**: just drop your custom matcher inside [`{}`](docs::gram_ref#block) block.
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
//! - [glossary](`docs::glossary`): glossary of terms and concepts.
//!
use std::ops::Range;

mod str;
#[cfg(doc)]
pub mod docs {
	pub mod glossary;
	/// grammar reference
	pub mod gram_ref;
}

/// check if a [`MatchAble`] matches an inline [expression](docs::gram_ref).
///
/// its syntax is `matches!(value: type, expr)`, where `value` is an expression evaluating to a [`MatchAble`], `type` specify the `MatchAble` type and `expr` is the grammer expression to match against.
///
/// captures are not allowed in `expr`, local variables can be used inside it.
///
/// returns `bool` if matches.
///
/// # example
/// ```
/// assert!(matches!("abc": str, 'a' 'b' 'c'));
/// assert!(!matches!("abd": str, 'a' 'b' 'c'));
/// let pat = "abc";
/// let value = String::from("abcabcabc");
/// assert!(matches!(value.as_str(): str, pat[3]))
/// ```
pub use gramex_macro::matches;

/// match a [`MatchAble`] against an inline [expression](docs::gram_ref).
///
/// its syntax is `try_match!(value: type, expr)`, where `value` is an expression evaluating to a [`MatchAble`], `type` specify the [`MatchAble`] type and `expr` is the grammer expression to match against.
///
/// captures are allowed in `expr`, local variables can be used inside it.
///
/// returns [`MatchResult`] of an implicit root capture type if there is captures, else [`MatchAble::Slice`] spanning all the input.
///
/// captures types can not be accessed in this release because of technical difficulty.
///
/// see [capture types](gramex#capture-types) in [`gramex`] macro documentation for more info.
///
/// ```
/// assert_eq!(try_match!("abc": str, 'a' 'b' 'c'), Ok("abc"));
/// assert!(try_match!("abd": str, 'a' 'b' 'c').is_err());
/// let pat = "bc";
/// let value = "abcdef";
/// let matched = try_match!(value: str, 'a' (bc = pat) 'd' (e = 'e') 'f').unwrap();
/// assert!(matched.matched == "abcdef" && matched.bc == "bc" && matched.e == "e");
/// ```
pub use gramex_macro::try_match;

/// declare a grammer module.
///
/// the `gramex` macro generate a set of matchers with their capture types for a specific [`MatchAble`] from a set of terms.
///
/// it consists of a header followed by a list of terms.
///
/// # header
/// ```
/// let header = mod_spec? 'for' type ';' use_decl*;
/// let mod_spec = vis 'mod' name;
/// ```
/// header is the first part of the macro body, it contains a `type` section that specifies the target [`MatchAble`] type.
///
/// a lifetime `'a` for the matched value is provided inside `type` if needed.
///
/// ```
/// gramex! { for Tokens<'a>; }
/// ```
///
/// the module `mod_spec` specifier is an optional part that specifies a module to create for the generated items, where `vis` is the module [visibility specifier](https://doc.rust-lang.org/stable/reference/visibility-and-privacy.html) and `name` is the module name.
///
/// if no module specifier is provided, the generated items are added to the current scope.
///
/// ```
/// gramex! { pub(crate) mod matchers for str; }
/// ```
///
/// use declerations can be added directly after the header if a module specifier is provided.
///
/// the items inside the current scope are all imported into the generated module.
///
/// `gramex` macro can be inserted locally inside fns and blocks but requires a module specifier.
/// ```
/// const A: &str = "a";
/// fn example () {
/// 	gramex! {
/// 		mod matchers for str;
/// 		let abc = A 'b' 'c';
/// 	}
/// 	assert!(matchers::match_abc("abc").is_ok());
/// }
/// ```
///
/// # terms
/// ```
/// let term = "let" name args? (':' type) = expr ("=>" map_block)? ';';
/// ```
/// terms are named expression statments that get generated into matchers with the name.
///
/// the requried parts of the term is its `name` and its root [grammer `expr`](docs::gram_ref).
///
/// `expr` can not reference local variables, but it can contain captures and reference other terms.
/// ```
/// gramex! {
/// 	mod matchers for str;
/// 	let abc = 'a' 'b' 'c';
/// 	let abcd = abc (d = 'd');
/// }
/// assert_eq!(matchers::match_abc("abc"), Ok("abc"));
/// let matched = matchers::match_abcd("abcd").unwrap();
/// assert!(matched.d == "d");
/// ```
///
/// term has an implicit root capture, the `type` and the `map_block` are optional parts that effect that capture, see [capture mapping](docs::gram_ref#capture-mapping) for more info.
/// ```
/// gramex! {
/// 	mod matchers for str;
/// 	let ABC: String = 'a' 'b' 'c' => {|v| v.to_uppercase()};
/// }
/// assert_eq!(matchers::match_ABC("abc"), Ok("ABC"));
/// ```
///
/// `args` is an optional list of [`Matcher`] arguments that the term accepts, it is a `,` separated list contained within `<>` brackets.
///
/// these arguments can be used inside `expr` by name, and become parameters in the generated term functions after the `value` argument.
///
/// the term become a [parameterized matcher](docs::glossary#parameterized-matcher) that is called using the [`call` atoms](docs::gram_ref#call).
/// ```
/// gramex! {
/// 	mod matchers for str;
/// 	let mbm<a, c> = a 'b' c;
/// 	let abc = mbm<'a', 'c'>;
/// }
/// assert_eq!(matchers::match_abc("abc"), Ok("abc"));
/// assert_eq!(matchers::match_mbm("abc", matcher_for('a'), matcher_for('c')), Ok("abc"));
/// ```
///
/// the term get generated into 3 functions:
/// - `fn name(value: &type, ...args, ind: &mut usize, status: &MatchStatus) -> MatchSignal`: a [`Matcher`] with the original name that doesnt capture.
/// - `fn capture_{name}(value: &type, ...args, ind: &mut usize, status: &MatchStatus) -> MatchResult<CapType>`: a [capturing function](docs::glossary#capturing-function) that match with capturing.    
/// it returns a [`MatchResult`] of the implicit root capture type if there is captures, else [`MatchAble::Slice`] spanning the matched input.
/// - `fn match_{name}(value: &type, ...args) -> MatchResult<CapType>`: a function that matches with capturing the whole input.   
/// it returns the same as `capture_{name}`, but fail on excess input.     
/// usefull when the term is the root pattern.
/// ```
/// gramex! {
/// 	mod matchers for str;
/// 	let abc = 'a' 'b' 'c';
/// }
/// assert_eq!(matchers::match_abc("abc"), Ok("abc"));
/// assert_eq!(matchers::abc("abc", &mut 0, &MatchStatus::default()), MatchSignal::Matched);
/// assert_eq!(matchers::capture_abc("abc", &mut 0, &MatchStatus::default()), Ok("abc"));
/// ```
///
/// # capture types
/// the captures types of each term is generated into a unique module `{name}_captures` inside the generated module / current scope.
///
/// it contains the captures own types, these types are [struct](docs::gram_ref#structured) / [enum](docs::gram_ref#enumumerated) named `Cap{Id}` containing a lifetime `'a` for the matched sections.
///
/// they implements [`Debug`](std::fmt::Debug) and [`PartialEq`] (requiring any nested capture to implement them).
///
/// the captures own type names are not stable (changes based on location), for that capture type maps are generated alongside.
///
/// these maps are modules named `CapType_types` each dedicated to a capture type, they reexport the nested captures own types based on their names, alongside their type map as `{name}_types` if they have it.
///
/// the type maps are not always generated, only if any of the nested captures has its own type.
///
/// alongside these types and maps, each module contains a `Root<'a>` type alias to the root capture resolved type.
///
/// they also contains a `RootCap<'a>` type alias to the root capture own type if needed, plus a `root_types` map for the root capture if needed.
///
/// ```
/// gramex! {
/// 	mod matchers for str;
/// 	let abc = 'a' 'b' 'c';
/// 	let cap: String = (a = 'a') (b = 'b' | (B = 'B')) (c1 = (c2 = (c3 = 'c'))) => {|v| v.matched.to_string()};
/// }
///
/// // example generation
/// mod matchers {
/// 	pub mod abc_captures {
///			pub type Root<'a> = &'a str;
/// 	}
/// 	pub mod cap_captures {
/// 		#[derive(Debug, PartialEq)]
/// 		pub struct Cap1<'a> {
/// 			pub matched: &'a str,
/// 			pub a: &'a str,
/// 			pub b: Cap2<'a>,
/// 			pub c1: Cap3<'a>,
/// 		}
/// 		#[derive(Debug, PartialEq)]
/// 		pub enum Cap2<'a> {
/// 			None,
/// 			B(&'a str),
/// 		}
/// 		#[derive(Debug, PartialEq)]
/// 		pub struct Cap3<'a> {
/// 			pub matched: &'a str,
/// 			pub c2: Cap4<'a>,
/// 		}
/// 		#[derive(Debug, PartialEq)]
/// 		pub struct Cap4<'a> {
/// 			pub matched: &'a str,
/// 			pub c3: &'a str,
/// 		}
/// 		pub mod Cap1_types {
/// 			pub type b<'a> = Cap2<'a>;
/// 			pub type c<'a> = Cap3<'a>;
/// 			pub use c1_types = Cap3_types;
/// 		}
/// 		pub mod Cap3_types {
/// 			pub type c2<'a> = Cap4<'a>;
/// 		}
/// 		pub type Root<'a> = String;
/// 		pub type RootCap<'a> = Cap1<'a>;
/// 		pub use Cap1_types as root_types;
/// 	}
/// }
/// ```
pub use gramex_macro::gramex;

/// create a [`Matcher`] from an inline [expression](docs::gram_ref).
///
/// its syntax is `matcher!(for type, expr)`, where `type` specify the target [`MatchAble`] type and `expr` is the grammer expression to match against.
///
/// captures are not allowed in `expr`, local variables can be used inside it.
///
/// # example
/// ```
/// assert!(matches("abc", matcher!(for str, 'a' 'b' 'c')));
/// assert!(!matches("abd", matcher!(for str, 'a' 'b' 'c')));
/// let pat = "abc";
/// let value = String::from("abcabcabc");
/// assert!(matches(value.as_str(), matcher!(for str, pat[3])))
/// ```
pub use gramex_macro::matcher;

/// a type that can be matched by gramex
///
/// `MatchAble` provider a common interface for all types matched using gramex.
///
/// it views the implementing type as a stream of tokens that can be sliced and accessed randomly.
///
/// # example for custom types
/// ```
/// enum Token {
/// 	Ident(String),
/// 	Nb(i64),
/// }
/// struct Tokens<'a>(&'a [Token]);
/// impl MatchAble for Tokens<'_> {
/// 	type Slice<'a> = Tokens<'a> where Self: 'a;
/// 	fn len(&self) -> usize {
/// 		self.0.len()
/// 	}
/// 	fn slice(&self, range: std::ops::Range<usize>) -> Tokens<'_> {
/// 		Tokens(&self.0[range])
/// 	}
/// 	fn skip_1(&self, ind: &mut usize, _status: &gramex::MatchStatus) -> gramex::MatchSignal {
/// 		if *ind + 1 > self.0.len() {
/// 			MatchSignal::InComplete
/// 		} else {
/// 			*ind += 1;
/// 			MatchSignal::Matched
/// 		}
/// 	}
/// }
/// ```
pub trait MatchAble {
	/// the type returned by [`MatchAble::slice`].
	///
	/// necessary for newtype pattern, must be linked to the lifetime of self.
	type Slice<'a>
	where
		Self: 'a;
	/// the length of the matchable token stream
	fn len(&self) -> usize;
	/// slice the matchable
	///
	/// the slice type shape is left to the implementer (by reference or by value), not the type itself.
	fn slice(&self, range: Range<usize>) -> Self::Slice<'_>;
	/// skip one token
	///
	/// necessary for some types like `str` where a token (`char`) can span multiple indicies
	fn skip_1(&self, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
/// handle matching by a specific type
///
/// `MatchBy` is where the matching logic lives, it is implemented on the [`MatchAble`] for each matching type.
///
/// the matching happens inside [`match_by`](MatchBy::match_by), it takes the `MatchAble`, matching type by value, the curent index and the [`MatchStatus`].
///
/// if the match is successful, the index is advanced to the next token and a [`MatchSignal::Matched`] is returned.
///
/// else the index is advance to where the match failed and one of the [`MatchSignal`] error variants is returned.
///
/// # example
/// ```
/// impl MatchBy<&str> for Tokens<'_> {
///		fn match_by(
///			&self, matcher: &str, ind: &mut usize, _status: &gramex::MatchStatus,
///		) -> MatchSignal {
///			if *ind + 1 > self.0.len() {
///				MatchSignal::InComplete
///			} else if let Token::Ident(ident) = &self.0[*ind]
///				&& ident == matcher
///			{
///				*ind += 1;
///				MatchSignal::Matched
///			} else {
///				MatchSignal::MisMatched
///			}
///		}
/// }
/// ```
pub trait MatchBy<T> {
	/// the matching logic
	fn match_by(&self, matcher: T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}

/// the result status of matching operation
///
/// `MatchStatus` is a type returned by every matching function to indicate if the match was successful or not with why.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum MatchSignal {
	#[default]
	/// the matching was successful.
	Matched,
	/// the matching failed for a mismatch between the tokens.
	MisMatched,
	/// the matching failed because the input is incomplete.
	InComplete,
	/// the matching failed because the matching succeeded but some tokens didnt get matched.
	Excess,
	/// the matching failed for some other reason.
	Error(String),
}
impl MatchSignal {
	/// convert to [`MatchError`].
	pub fn into_err(self, ind: usize) -> MatchError {
		match self {
			Self::Matched => MatchError::other(format!("being normal at {ind}"), ind),
			Self::MisMatched => MatchError::mismatch(ind),
			Self::InComplete => MatchError::incomplete(ind),
			Self::Excess => MatchError::excess(ind),
			Self::Error(msg) => MatchError::other(msg, ind),
		}
	}
	/// check if the [`MatchSignal`] is an error.
	pub fn is_err(&self) -> bool {
		!core::matches!(self, Self::Matched)
	}
}

/// the status of matching.
///
/// contains variables / flags affecting the matching operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchStatus {
	/// whether the failure of the current match halt the overall matching.
	///
	/// set to `true` in all expressions except in negation, peak, or, and loop iterations after minimuim.
	///
	/// used to provide better error messages when needed.
	pub in_main_path: bool,
}
impl Default for MatchStatus {
	fn default() -> Self {
		Self { in_main_path: true }
	}
}

/// an error encountered during matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchError {
	/// the message of the error.
	pub msg: String,
	/// the index where the error occured.
	pub ind: usize,
}
impl MatchError {
	/// create a new `MatchError` with mismatch as the message at specified index.
	pub fn mismatch(ind: usize) -> Self {
		Self { msg: format!("mismatch at {ind}"), ind }
	}
	/// create a new `MatchError` with incomplete as the message at specified index.
	pub fn incomplete(ind: usize) -> Self {
		Self { msg: format!("incomplete input at {ind}"), ind }
	}
	/// create a new `MatchError` with excess as the message at specified index.
	pub fn excess(ind: usize) -> Self {
		Self { msg: format!("excess input at {ind}"), ind }
	}
	/// create a new `MatchError` with custom message at specified index.
	///
	/// doesnt append the index to the message.
	pub fn other(msg: String, ind: usize) -> Self {
		Self { msg, ind }
	}
}
impl std::fmt::Display for MatchError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.msg)
	}
}

/// a [`Result`] type dedicated for matching.
///
/// convertable to [`MatchSignal`].
pub type MatchResult<T> = Result<T, MatchError>;
impl<T> From<MatchResult<T>> for MatchSignal {
	fn from(value: MatchResult<T>) -> Self {
		match value {
			Ok(_) => MatchSignal::Matched,
			Err(err) => MatchSignal::Error(err.msg),
		}
	}
}

/// a function that matches a [`MatchAble`].
///
/// takes the `MatchAble`, the current index and [`MatchStatus`].
///
/// if it succeeded, it advance the index to the next token and return [`MatchSignal::Matched`].
///
/// else, it advance the index to where the match failed and return one of the [`MatchSignal`] error variants.
///
/// # example
/// ```
/// assert!(matches("abc", matcher!(for str, "abc")));
/// assert!(matches("abc", by(|v, i, s| v.match_by("abc", i, s))));
/// assert!(matches("a", a(|v| v == 'a'));
/// ```
pub trait Matcher<T: MatchAble + ?Sized>: Fn(&T, &mut usize, &MatchStatus) -> MatchSignal {
	fn do_match(&self, matchable: &T, ind: &mut usize, status: &MatchStatus) -> MatchSignal;
}
impl<T: MatchAble + ?Sized, M: Matcher<T>> MatchBy<M> for T {
	fn match_by(&self, matcher: M, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		matcher.do_match(self, ind, status)
	}
}
// tried with a general matcher signature that supports every function returning a thing convertable to `MatchSignal` to support passing raw capturing matchers but faced lifetime madness.
impl<T: MatchAble + ?Sized, F> Matcher<T> for F
where
	F: for<'a> Fn(&'a T, &mut usize, &MatchStatus) -> MatchSignal,
{
	fn do_match(&self, matchable: &T, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		self(matchable, ind, status)
	}
}

pub fn by<T: MatchAble + ?Sized, F: Fn(&T, &mut usize, &MatchStatus) -> MatchSignal>(
	matcher: F,
) -> F {
	matcher
}
#[doc(hidden)]
pub mod __private {
	/// map block type inference support
	pub fn conv<T, U>(cap: T, conv: impl Fn(T) -> U) -> U {
		conv(cap)
	}
}
