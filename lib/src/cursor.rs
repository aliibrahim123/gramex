//! gramex's imperative mode for advanced parsing.
//!
//! the `cursor` module provides a solution for cases where [grammar expressions](crate::gram_ref) are not enough: non-trivial error handling, highly custom parsing operations, advanced recovery strategies, context-sensitive parsing, and fully visible matching logic for determinism and efficiency.
//!
//! the core of this module is [`Cursor`], a `(MatchAble, offset)` tuple that acts as a lightweight matching context, passed around the matching logic and providing common, helpful utilities.
//!
//! the [`Cursor`] provides the typical unconstrained imperative experience of Rust with all its features, while enriching it with the power, universality and expressiveness of [`Matcher`] through a set of utilities.
//!
//! # quick overview
//! at its core, a [`Cursor`] is a tuple of [`Matcher::do_match`] inputs. it hides all manual offset handling and exposes only a set of declarative matching actions that reflect the [`Mode`] [tiers](crate::modes), with the cursor's own twist: [`eat`](Cursor::eat), [`try_eat`](Cursor::try_eat) and [`test`](Cursor::test).
//! ```
//! let cur = MyCursor::new("input");
//! // `eat`: match with a `Matcher` in `Parse` mode
//! cur.eat("let")?;
//!
//! // `try_eat`: atomically match in `Capture` mode
//! if let Some(nb) = cur.try_eat(nb) {
//!     Expr::Nb(nb)
//! }
//!
//! // `test`: match in `Test` mode without advancing
//! if cur.test('[') {
//!     parse_arr(cur)
//! }
//! ```
//!
//! each of these methods also has a macro equivalent that takes a [grammar expression](crate::gram_ref):
//! ```
//! // uuid
//! let (a, b, c, d, e) = eat!(cur,
//!     (a = hex[8]) '-' (b = hex[4]) '-' (c = hex[4]) '-' (d = hex[4]) '-' (e = hex[12])
//! )?;
//!
//! if let Some(nb) = try_eat!(cur, '-'? ('0'..'9')+) {
//!     Expr::Nb(nb.parse().unwrap())
//! }
//!
//! let is_sign = test!(cur, '-' | '+');
//!
//! // `match_map`: grammar-based match expression
//! match_map!(cur, {
//!     '+' => Expr::Plus,
//!     '-' => Expr::Minus,
//!     '*' => Expr::Mult,
//!     '/' => Expr::Div,
//!     // default case
//!     else => break,
//! })
//! ```
//!
//! in addition, [`Cursor`] provides several methods that simplify manual matching logic.
//! ```
//! // `peek`: get the current token
//! if let Some(Token::Nb(nb)) = cur.peek() {
//!     // skip one token
//!     cur.skip();
//!     Expr::Nb(nb)
//! } else {
//!     // `MatchError::expected` at the current offset
//!     cur.expected("number")
//! }
//!
//! while !cur.is_end() {
//!     do_something(cur)
//! }
//!
//! if cur.try_eat("something").is_none() {
//!     // `rewind`: set the offset
//!     cur.rewind(start);
//! }
//! ```
//!
//! [`Cursor`] is usually used with custom enum-based token lists, whose matchers can be generated automatically by the [`derive`](crate::derive) module.
//! ```
//! #[derive_enum_matcher]
//! #[derive(Debug, PartialEq)]
//! enum Token {
//!     Nb(i64),
//!     Add,
//!     Eq
//!     // ..
//! }
//! #[derive(Debug)]
//! struct Tokens<'src>(&'src [Token]);
//! derive_slice_matchable!(Tokens(&[Token]), matchers: true, expected: Debug);
//! type MyCursor<'src> = SimpleCursor<'src, Tokens>;
//!
//! let mut cur = MyCursor::new(&Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]));
//! let exprs = vec![*cur.eat(nb)?];
//! while cur.try_eat(add).is_some() {
//!     exprs.push(*cur.eat(nb)?);
//! }
//! assert_eq!(exprs, vec![1, 2]);
//! ```

use core::{
	fmt::{self, Display, Formatter},
	ops::Add,
};

use crate::{
	MatchAble, Matcher, Mode,
	core::{Check, Test},
	result::{Expected, MatchError, MatchResult},
};

/// match with a [grammar expression](crate::gram_ref) at the current offset.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `eat` macro takes the [`Cursor`] and the expression, matches it in [`Parse`](crate::modes::Parse) [`Mode`] and returns `Result<Capture, Cursor::Error>`.
///
/// it is the macro equivalent of [`Cursor::eat`].
///
/// # example
/// ```
/// let mut cur = SimpleCursor::new("abc");
/// assert_eq!(eat!(&mut cur, a:'a' 'b'? c*:'c'), Ok(("a", vec!["c"])));
/// assert!(eat!(&mut cur, 'a').is_err());
/// ```
#[cfg(feature = "macros")]
pub use gramex_macro::eat;

/// a [grammar](crate::gram_ref)-based match expression for [`Cursor`].
///
/// ```gramex
/// let arm = pat:expr "=>" map:rust_expr;
/// let body = cur:rust_expr ',' '{' arm* ("else" "=>" rust_expr)? '}';
/// ```
///
/// the `match_map` macro takes the [`Cursor`], a list of `pat => map` arms and, optionally, a final `else` arm; it evaluates to the type of `map`.
///
/// it atomically tries to match each `pat` in order, in [`Capture`](crate::modes::Capture) [`Mode`]; on the first match, its `map` expression is evaluated as the result, with [captures](crate::gram_ref#captures) bound by their names.
///
/// if no arm matches, it evaluates the `else` arm if present, and otherwise panics with [`unreachable!`].
///
/// # example
/// ```
/// fn map(cur: &mut SimpleCursor<str>) -> Case {
///     match_map!(cur, {
///         'a' => Case::A,
///         'b' c1:_ ',' c2:_ => Case::B(c1, c2),
///         'd' | 'e' => Case::D_E,
///         else => Case::F,
///     })
/// }
/// assert_eq!(map(&mut SimpleCursor::new("a")), Case::A);
/// assert_eq!(map(&mut SimpleCursor::new("b1,2")), Case::B("1", "2"));
/// assert_eq!(map(&mut SimpleCursor::new("d")), Case::D_E);
/// assert_eq!(map(&mut SimpleCursor::new("e")), Case::D_E);
/// assert_eq!(map(&mut SimpleCursor::new("f")), Case::F);
/// ```
#[cfg(feature = "macros")]
pub use gramex_macro::match_map;

/// match with a [grammar expression](crate::gram_ref) at the current offset, without advancing.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `test` macro takes the [`Cursor`] and the expression, matches it in [`Test`] [`Mode`] and returns a `bool` as the result.
///
/// it is the macro equivalent of [`Cursor::test`].
///
/// # example
/// ```
/// let mut cur = SimpleCursor::new("abc");
/// let off = cur.offset();
/// assert!(test!(&mut cur, "abc"));
/// assert_eq!(cur.offset(), off);
/// assert!(!test!(&mut cur, "abd"));
/// assert_eq!(cur.offset(), off);
/// ```
#[cfg(feature = "macros")]
pub use gramex_macro::test;

/// try to match with a [grammar expression](crate::gram_ref) at the current offset.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `try_eat` macro takes the [`Cursor`] and the expression, matches it in [`Capture`](crate::modes::Capture) [`Mode`] and returns an `Option<Capture>`.
///
/// it is the macro equivalent of [`Cursor::try_eat`].
///
/// # example
/// ```
/// let mut cur = SimpleCursor::new("abcd");
/// assert_eq!(try_eat!(&mut cur, a:'a' 'b'? c*:'c'), Some(("a", vec!["c"])));
/// let off = cur.offset();
/// assert_eq!(try_eat!(&mut cur, "de"), None);
/// assert_eq!(cur.offset(), off);
/// ```
#[cfg(feature = "macros")]
pub use gramex_macro::try_eat;

/// a simple span for simple cases.
///
/// `SimpleSpan` is a data structure that represents a range in the source.
///
/// it is used when you just need a span as simple as a [`Range`](core::ops::Range).
///
/// **point span**: a span covering exactly one unit-sized token.
///
/// # example
/// ```
/// let span = SimpleSpan::new(1, 3);
/// assert_eq!((span.start, span.end), (1, 3));
/// assert_eq!(span.to_string(), "1..3");
///
/// assert!(!span.is_point());
/// assert!(SimpleSpan::point(1).is_point());
///
/// assert!(span.join(&SimpleSpan::new(2, 4)) == SimpleSpan::new(1, 4));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleSpan<T = usize> {
	/// the start offset of the span
	pub start: T,
	/// the end offset of the span, exclusive
	pub end: T,
}
impl<T: Clone + PartialEq + Ord + Add<Output = T> + From<u8>> SimpleSpan<T> {
	/// create a new span.
	///
	/// # example
	/// ```
	/// assert_eq!(SimpleSpan::new(1, 3), SimpleSpan { start: 1, end: 3 });
	/// ```
	pub fn new(start: T, end: T) -> Self {
		Self { start, end }
	}

	/// create a point span.
	///
	/// # example
	/// ```
	/// assert_eq!(SimpleSpan::point(1), SimpleSpan { start: 1, end: 2 });
	/// ```
	pub fn point(ind: T) -> Self {
		Self::new(ind.clone(), ind + 1.into())
	}

	/// check whether the span is a point span.
	///
	/// # example
	/// ```
	/// assert!(SimpleSpan::point(1).is_point());
	/// assert!(!SimpleSpan::new(1, 5).is_point());
	/// ```
	pub fn is_point(&self) -> bool {
		self.start.clone() + 1.into() == self.end
	}

	/// join two spans, from the smallest start to the largest end, even if they are disjoint.
	///
	/// # example
	/// ```
	/// assert_eq!(SimpleSpan::join(&SimpleSpan::new(1, 3), &SimpleSpan::new(2, 4)), SimpleSpan::new(1, 4));
	/// assert_eq!(SimpleSpan::join(&SimpleSpan::new(1, 3), &SimpleSpan::new(1, 4)), SimpleSpan::new(1, 4));
	/// assert_eq!(SimpleSpan::join(&SimpleSpan::new(1, 4), &SimpleSpan::new(2, 3)), SimpleSpan::new(1, 4));
	/// assert_eq!(SimpleSpan::join(&SimpleSpan::new(1, 1), &SimpleSpan::new(3, 4)), SimpleSpan::new(1, 4));
	/// ```
	pub fn join(&self, other: &Self) -> Self {
		Self::new(
			self.start.clone().min(other.start.clone()),
			self.end.clone().max(other.end.clone()),
		)
	}
}
impl Display for SimpleSpan {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}..{}", self.start, self.end)
	}
}

/// capture the span of what `matcher` matches.
///
/// `span_around` takes a [`Matcher`] and returns a [`Matcher`] that matches with it, propagating its error and capturing the span of the section it matched.
///
/// # example
/// ```
/// assert_eq!(
///     try_match!("abcd", 'a' span:{span_around("bc")} 'd'),
///     Some((SimpleSpan::new(1, 3),))
/// );
/// assert_eq!(try_match!("abcd", 'a' span:{span_around("bd")} 'd'), None);
/// ```
pub fn span_around<T: MatchAble + ?Sized, M: Matcher<T>>(matcher: M) -> SpanAround<M> {
	SpanAround(matcher)
}

/// the [`span_around`] matcher
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanAround<M>(M);
impl<T: MatchAble + ?Sized, U: Matcher<T>> Matcher<T> for SpanAround<U> {
	type Capture<'src>
		= SimpleSpan
	where
		T: 'src;
	fn do_match<M: Mode>(
		&self, matched: &T, off: &mut usize,
	) -> MatchResult<SimpleSpan, M> {
		let start = *off;
		self.0.do_match::<M>(matched, off)?;
		Ok(M::wrap_success(SimpleSpan::new(start, *off)))
	}
}

/// a matching cursor for a [`MatchAble`].
///
/// `Cursor` is a `(value, offset)` tuple that provides an imperative matching experience, enhanced by and integrated with the rest of the gramex ecosystem.
///
/// for more info, read the [module documentation](crate::cursor).
///
/// # implementation guide
/// to implement `Cursor` for a custom type, you must specify the [`MatchAble`] and provide the input and the offset through [`input`](Self::input), [`off`](Self::off) and [`off_mut`](Self::off_mut).
///
/// in addition, you must specify the `Cursor`'s [`Error`](Self::Error) type, along with its mapping from [`MatchError`] through [`map_error`](Self::map_error).
///
/// the `Cursor` has a `'src` lifetime that separates the lifetime of the input [slices](MatchAble::Slice) from the `Cursor`'s own mutations.
/// ```
/// struct MyCursor<'src> {
///     // the input lives outside the cursor, so capture lifetimes are independent of cursor mutation
///     value: &'src Tokens<'src>,
///     off: usize,
///     // supports multiple errors
///     errors: Vec<String>,
/// }
/// impl<'src> Cursor<'src> for MyCursor<'src> {
///     type MatchAble = Tokens<'src>;
///     type Error = ();
///     fn input(&self) -> &'src Self::MatchAble {
///         self.value
///     }
///     fn off(&self) -> usize {
///         self.off
///     }
///     fn off_mut(&mut self) -> &mut usize {
///         &mut self.off
///     }
///     fn map_error(&mut self, err: MatchError) -> () {
///         self.errors.push(err.to_string());
///         ()
///     }
/// }
/// ```
///
/// if you just need a simple cursor with unmodified [`MatchError`]s, see [`SimpleCursor`].
pub trait Cursor<'src> {
	/// the [`MatchAble`] the `Cursor` works on.
	type MatchAble: MatchAble + ?Sized + 'src;
	/// the `Cursor`'s custom error type.
	type Error;

	/// return a reference to the [`MatchAble`] the `Cursor` works on.
	///
	/// this reference is unaffected by `Cursor` mutations.
	fn input(&self) -> &'src Self::MatchAble;

	/// return the current offset.
	fn off(&self) -> usize;
	/// return a mutable reference to the offset.
	fn off_mut(&mut self) -> &mut usize;

	/// convert a [`MatchError`] to the `Cursor`'s [`Error`](Self::Error) type.
	fn map_error(&mut self, err: MatchError) -> Self::Error;

	/// get the [token](MatchAble::Token) at the current offset.
	///
	/// `peek` uses [`MatchAble::get_token`] under the hood.
	///
	/// it returns `None` at the end of the input.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("a");
	/// assert_eq!(cur.peek(), Some('a'));
	/// cur.skip();
	/// assert_eq!(cur.peek(), None);
	/// ```
	#[inline]
	fn peek(&self) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		self.input().get_token(self.off())
	}

	/// get the [token](MatchAble::Token) `n` tokens ahead of the current one.
	///
	/// `peek_next` uses [`MatchAble::skip_n`] under the hood.
	///
	/// it returns `None` at the end of the input.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.peek_next(0), Some('a'));
	/// assert_eq!(cur.peek_next(1), Some('b'));
	/// assert_eq!(cur.peek_next(2), Some('c'));
	/// assert_eq!(cur.peek_next(3), None);
	/// ```
	#[inline]
	fn peek_next(&self, n: usize) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		let input = self.input();
		let mut off = self.off();
		input.skip_n::<Test>(&mut off, n).ok()?;
		input.get_token(off)
	}

	/// check whether the `Cursor` is at the end of the input.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("a");
	/// assert!(!cur.is_end());
	/// cur.skip();
	/// assert!(cur.is_end());
	/// ```
	#[inline]
	fn is_end(&self) -> bool {
		self.off() == self.input().len()
	}

	/// match with a [`Matcher`] at the current offset.
	///
	/// `eat` uses [`Parse`](crate::modes::Parse) [`Mode`] and returns the `Cursor`'s [`Error`](Self::Error) instead of a [`MatchError`].
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.eat('a'), Ok("a"));
	/// assert_eq!(cur.eat("bc"), Ok("bc"));
	/// assert!(cur.eat("d").is_err());
	/// ```
	#[inline]
	fn eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Result<M::Capture<'src>, Self::Error> {
		matcher.parse(self.input(), self.off_mut()).map_err(|err| self.map_error(err))
	}

	/// try to match with a [`Matcher`] at the current offset.
	///
	/// `try_eat` uses [`Capture`](crate::modes::Capture) [`Mode`].
	///
	/// it is atomic: on failure, it returns `None` and leaves the offset unchanged.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.try_eat('a'), Some("a"));
	/// let off = cur.off();
	/// assert_eq!(cur.try_eat("a"), None);
	/// assert_eq!(cur.off(), off);
	/// ```
	#[inline]
	fn try_eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Option<M::Capture<'src>> {
		let mut off = self.off();
		matcher.capture(self.input(), &mut off).inspect(|_| *self.off_mut() = off)
	}

	/// match with a [`Matcher`] at the current offset, without advancing.
	///
	/// `test` uses [`Test`] [`Mode`] and returns `false` on failure.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// let off = cur.off();
	/// assert!(cur.test('a'));
	/// assert_eq!(cur.off(), off);
	/// assert!(!cur.test("b"));
	/// assert_eq!(cur.off(), off);
	/// ```
	#[inline]
	fn test(&self, matcher: impl Matcher<Self::MatchAble>) -> bool {
		matcher.test(self.input(), &mut self.off())
	}

	/// capture the next token and advance past it.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.eat_token(), Ok('a'));
	/// assert_eq!(cur.eat_token(), Ok('b'));
	/// assert_eq!(cur.eat_token(), Ok('c'));
	/// assert!(cur.eat_token().is_err());
	/// ```
	#[inline]
	fn eat_token(
		&mut self,
	) -> Result<<Self::MatchAble as MatchAble>::Token<'src>, Self::Error> {
		let start = self.off();
		self.input().skip_n::<Check>(self.off_mut(), 1).map_err(|e| self.map_error(e))?;
		Ok(self.input().get_token(start).unwrap())
	}

	/// try to capture the next token and advance past it.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.try_eat_token(), Some('a'));
	/// assert_eq!(cur.try_eat_token(), Some('b'));
	/// assert_eq!(cur.try_eat_token(), Some('c'));
	/// assert!(cur.try_eat_token().is_none());
	/// ```
	#[inline]
	fn try_eat_token(&mut self) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		let start = self.off();
		self.input().skip_n::<Test>(self.off_mut(), 1).ok()?;
		self.input().get_token(start)
	}

	/// advance the `Cursor` by one token.
	///
	/// `skip` uses [`MatchAble::skip_n`] under the hood, and is a no-op at the end of the input.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("ab");
	/// assert_eq!(cur.peek(), Some('a'));
	/// cur.skip();
	/// assert_eq!(cur.peek(), Some('b'));
	/// cur.skip();
	/// assert!(cur.is_end());
	/// ```
	#[inline]
	fn skip(&mut self) {
		_ = self.input().skip_n::<Test>(self.off_mut(), 1);
	}

	/// rewind the `Cursor` to the given offset.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// let start = cur.off();
	/// cur.eat("ab");
	/// if cur.test('c') {
	///     cur.rewind(start);
	/// }
	/// assert!(cur.test('a'));
	/// ```
	#[inline]
	fn rewind(&mut self, off: usize) {
		*self.off_mut() = off;
	}

	/// generate a [`MatchError`] with the given [`Expected`].
	///
	/// it generates a [mismatch](crate::result::MatchErrorKind::MisMatch) or [incomplete](crate::result::MatchErrorKind::InComplete) [`MatchError`], depending on whether the `Cursor` is at the end of the input, then maps it through [`map_error`](Self::map_error).
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("a");
	/// assert_eq!(
	///     cur.expected::<()>("b"),
	///     Err(MatchError::mismatch("b".into(), 0))
	/// );
	/// cur.skip();
	/// assert_eq!(
	///     cur.expected::<()>("b"),
	///     Err(MatchError::incomplete("b".into(), 1))
	/// );
	/// ```
	fn expected<T>(&mut self, expected: impl Into<Expected>) -> Result<T, Self::Error> {
		let err = match self.is_end() {
			true => MatchError::incomplete(expected.into(), self.off()),
			false => MatchError::mismatch(expected.into(), self.off()),
		};
		Err(self.map_error(err))
	}
}

/// a simple [`Cursor`] for simple cases.
///
/// `SimpleCursor` is the simplest possible [`Cursor`]: a plain `(input, off)` tuple with the regular [`MatchError`] and no extra bloat.
///
/// # example
/// ```
/// let mut cur = SimpleCursor::new("abc");
/// assert_eq!(cur.eat('a'), Ok("a"));
/// assert_eq!(cur.eat("a"), Err(MatchError::mismatch("a".into(), 1)));
///
/// assert_eq!(cur.peek(), Some('b'));
/// assert_eq!(cur.try_eat('b'), Some("b"));
/// assert!(!cur.test('d'));
///
/// cur.skip();
/// assert!(cur.is_end());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleCursor<'src, T: MatchAble + ?Sized + 'src> {
	/// the wrapped input
	pub input: &'src T,
	/// the current offset
	pub off: usize,
}
impl<'src, T: MatchAble + ?Sized> SimpleCursor<'src, T> {
	/// create a new `SimpleCursor` wrapping the given input.
	pub fn new(input: &'src T) -> Self {
		Self { input, off: 0 }
	}
}
impl<'src, T: MatchAble + ?Sized> Cursor<'src> for SimpleCursor<'src, T> {
	type MatchAble = T;
	type Error = MatchError;
	fn input(&self) -> &'src Self::MatchAble {
		self.input
	}
	fn off(&self) -> usize {
		self.off
	}
	fn off_mut(&mut self) -> &mut usize {
		&mut self.off
	}
	fn map_error(&mut self, err: MatchError) -> Self::Error {
		err
	}
}
