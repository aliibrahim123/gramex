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
//! - term based grammer defenition thought [`gramex`], or inlined expression matching through [`matches`](matches!) and [`try_match`]
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
//! // `cond -> expr` matches `expr` if `cond` matches
//! assert!(matches!("ad": str, 'a' ('b' -> "bc") 'd'));
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

pub mod bits;
pub mod str;
mod utility;
pub use utility::*;
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
/// can pass any type implementing [`AsRef<type>`] by ref or value, returns `true` if matches.
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
/// can pass any type implementing [`AsRef<type>`] by ref or value.
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
/// assert_eq!(matchers::match_mbm("abc", matcher_for(&'a'), matcher_for(&'c')), Ok("abc"));
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
/// tokens doesnt need to be uniformally sized.
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
/// 	fn get_n(
///			&self, ind: &mut usize, n: usize, _status: &gramex::MatchStatus,
///		) -> Result<Tokens<'_>, MatchSignal> {
/// 		if *ind + n > self.0.len() {
/// 			Err(MatchSignal::InComplete)
/// 		} else {
/// 			*ind += n;
/// 			Ok(Tokens(&self.0[*ind - n..*ind]))
/// 		}
/// 	}
/// }
/// ```
///
/// # note
/// my need to implement [`AsRef<Self>`] for `Self` to use inside [`matches!`] and [`try_match!`].
pub trait MatchAble {
	/// the type returned by [`MatchAble::slice`].
	///
	/// necessary for newtype pattern, must be linked to the lifetime of self.
	type Slice<'a>
	where
		Self: 'a;
	/// the length of the matchable token stream.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(123), Token::Ident("abc".to_string()), Token::Nb(456)]);
	/// assert_eq!(tokens.len(), 3);
	/// ```
	fn len(&self) -> usize;
	/// slice the matchable.
	///
	/// the slice type shape is left to the implementer (by reference or by value), not the type itself.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(123), Token::Ident("abc".to_string()), Token::Nb(456)]);
	/// assert_eq!(tokens.slice(1..3), Tokens(&[Token::Ident("abc".to_string()), Token::Nb(456)]));
	/// ```
	fn slice(&self, range: Range<usize>) -> Self::Slice<'_>;
	/// get a slice of n tokens from the matchable.
	///
	/// return a [`Result`] of [`MatchAble::Slice`] and advance the index to the next token if matched, else return a [`MatchSignal`].
	///
	/// necessary for some types (like `str`) that doesnt have uniform token size.
	///
	/// has a default implementation for 1 sized tokens.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(123), Token::Ident("abc".to_string()), Token::Nb(456)]);
	/// assert_eq!(tokens.get_n(&mut 0, 1, &MatchStatus::default()), Ok(Tokens(&[Token::Nb(123)])));
	/// ```
	#[inline]
	fn get_n(
		&self, ind: &mut usize, n: usize, status: &MatchStatus,
	) -> Result<Self::Slice<'_>, MatchSignal> {
		// ignore status
		let _ = status;
		if *ind + n > self.len() {
			Err(MatchSignal::InComplete)
		} else {
			*ind += n;
			Ok(self.slice(*ind - n..*ind))
		}
	}
	/// skip n tokens from the matchable.
	///
	/// has a default implementation that calls [`MatchAble::get_n`] and ignore the result.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(123), Token::Ident("abc".to_string()), Token::Nb(456)]);
	/// let mut ind = 0;
	/// tokens.skip_n(&mut ind, 1, &MatchStatus::default());
	/// assert_eq!(ind, 1);
	/// ```
	#[inline]
	fn skip_n(&self, ind: &mut usize, n: usize, status: &MatchStatus) -> MatchSignal {
		match self.get_n(ind, n, status) {
			Ok(_) => MatchSignal::Matched,
			Err(sig) => sig,
		}
	}
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
/// every [`MatchAble`] implement [`MatchBy`] for `Matcher`.
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

/// check if a [`MatchAble`] matches against a [`Matcher`].
///
/// like [`matches!`] but normal fn not macro.
///
/// # example
/// ```
/// assert!(matches("abc", matcher_for("abc")));
/// ```
pub fn matches<T: MatchAble + ?Sized>(value: &T, matcher: impl Matcher<T>) -> bool {
	let mut ind = 0;
	let sig = matcher.do_match(value, &mut ind, &MatchStatus::default());
	if ind != value.len() { false } else { sig == MatchSignal::Matched }
}

#[doc(hidden)]
pub mod __private {
	/// map block type inference support
	pub fn conv<T, U>(cap: T, conv: impl Fn(T) -> U) -> U {
		conv(cap)
	}
}
