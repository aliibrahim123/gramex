//! gramex imperative mode for advance parsing.
//!
//! cursor module provide a solution for cases where the [grammar expressions](crate::gram_ref) are not enough: non basic error side, highly custom parsing operations, advance recovery strategies, context sensitive parsing, fully visible matching logic for deterministic and efficincy.
//!
//! the core of this module is [`Cursor`], a `(MatchAble, offset)` tuple that act as a lightweight matching context passed around the matching logic and providing helpful and common utilities.
//!
//! the [`Cursor`] provide the typical unconstrained imperative experience of rust with all its features, while enriching it with the power of [`Matcher`] and its universality and expressivety through a set of utilities.
//!
//! # quick overview
//! at its core, a [`Cursor`] is a tuple of [`Matcher::do_match`] inputs, it hide all the manual offset handling and expose only a set of declarative matching actions reflecting the [`Mode`] [tiers](crate::modes) with cursor own twist: [`eat`](Cursor::eat), [`try_eat`](Cursor::try_eat), and [`test`](Cursor::test).
//! ```
//! let cur = MyCursor::new("input");
//! // `eat`: match by a `Matcher` with `Parse` mode
//! cur.eat("let")?;
//!
//! // `try_eat`: atomicly match with `Capture` mode
//! if let Some(nb) = cur.try_eat(nb) {
//!     Expr::Nb(nb)
//! }
//!
//! /// `test`: match with `Test` mode without advancing
//! if cur.test('[') {
//!     parse_arr(cur)
//! }
//! ```
//!
//! these methods also have a macro equivalent taking a [grammer expression](crate::gram_ref)
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
//! // `match_map`: grammar based match expression
//! match_map!(cur, {
//!     '+' => Expr::Plus,
//!     '-' => Expr::Minus
//!     '*' => Expr::Mult,
//!     '/' => Expr::Div
//!     // default case
//!     else => break,
//! })
//! ```
//!
//! in addition, [`Cursor`] also provide several methods to simplify manual matching logic.
//! ```
//! // `peek`: get current token
//! if let Some(Token::Nb(nb)) = cur.peek() {
//!     // skip 1 token
//!     cur.skip();
//!     Expr::Nb(nb)
//! } else {
//!     // `MatchError::expected` at current offset
//!     cur.expected("number")
//! }
//!
//! while !cur.is_end() {
//!     do_something(cur)
//! }
//!
//! if cur.try_eat("something").is_none() {
//!     // `rewind`: set offset
//!     cur.rewind(start);
//! }
//! ```
//!
//! [`Cursor`] is usually used with custom enum based token list, which can be automaticly generated with the [`derive`](crate::derive) module.
//! ```
//! #[derive_enum_matcher]
//! #[derive(Debug, PartialEq)]
//! enum Token {
//!     Nb(i64),
//!     Add,
//!     Eq
//!     // ..
//! }
//! struct Tokens<'src>(&'src [Token]);
//! derive_slice_matchable!(Tokens(&[Token]), true);
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
	core::Test,
	result::{Expected, MatchError, MatchResult},
};

/// match by a [grammar expression](crate::gram_ref) at the current offset.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `eat` macro takes the [`Cursor`] and the expression, match it with [`Parse`](crate::modes::Parse) [`Mode`] and return `Result<Capture, Cursor::Error>`.
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

/// [grammar](crate::gram_ref) based match expression for [`Cursor`].
///
/// ```gramex
/// let arm = pat:expr "=>" map:rust_expr;
/// let body = cur:rust_expr ',' '{' arm* ("else" "=>" rust_expr)? '}';
/// ```
///
/// the `match_map` macro takes the [`Cursor`] and a list of `(pat => map)` arms, and optionally an `else` arm at the end, and evaluate to the type of `map`.
///
/// it try match atomicly each `pat` in order with [`Capture`](crate::modes::Capture) [`Mode`], on first match, its `map` expression get evaluated as a result with [captures](crate::gram_ref#captures) binded by their name.
///
/// else it evaluate the `else` arm as result if found, else it panic with [`unreachable!`].
///
/// # example
/// ```
/// fn map(cur: &mut SimpleCursor<str>) -> Case {
///     match_map!(cur, {
///         'a' => Case::A,
///         'b' c1:_ ',' c2:_ => Case::B(c1, c2),
///         'd' | 'e' => Case::D_E,
///         else => Case:F,
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

/// match by a [grammar expression](crate::gram_ref) at the current offset without advancing.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `test` macro takes the [`Cursor`] and the expression, match it with [`Test`] [`Mode`] and return `bool` as the result.
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

/// try match by a [grammar expression](crate::gram_ref) at the current offset.
///
/// ```gramex
/// let body = cur:rust_expr ',' expr;
/// ```
///
/// the `try_eat` macro takes the [`Cursor`] and the expression, match it with [`Capture`](crate::modes::Capture) [`Mode`] and return `Option<Capture>`.
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
/// `SimpleSpan` is a data strucure that represent a range of source code.
///
/// it is used when you just need a span as simple as a [`Range`](core::ops::Range).
///
/// **point span**: span spanning 1 unitary token.
///
/// # example
/// ```
/// let span = SimpleSpan::new(1, 3);
/// assert_eq!((span.start, span.end), (1, 3));
/// assert_eq!(span.to_string(), "1..3");
///
/// assert!(!span.is_point());
/// assert!(Span::point(1).is_point());
///
/// assert!(span.join(&Span::new(2, 4)) == Span::new(1, 4));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleSpan<T = usize> {
	/// the start offset of the span
	pub start: T,
	/// the end offset of the span, exclusive
	pub end: T,
}
impl<T: Clone + PartialEq + Ord + Add<Output = T> + From<u8>> SimpleSpan<T> {
	/// create a new span
	///
	/// # example
	/// ```
	/// assert_eq!(SimpleSpan::new(1, 3), SimpleSpan { start: 1, end: 3 });
	/// ```
	pub fn new(start: T, end: T) -> Self {
		Self { start, end }
	}

	/// create a point span
	///
	/// # example
	/// ```
	/// assert_eq!(SimpleSpan::point(1), SimpleSpan { start: 1, end: 2 });
	/// ```
	pub fn point(ind: T) -> Self {
		Self::new(ind.clone(), ind + 1.into())
	}

	/// check if the span is a point span
	///
	/// # example
	/// ```
	/// assert!(SimpleSpan::point(1).is_point());
	/// assert!(!SimpleSpan::new(1, 5).is_point());
	/// ```
	pub fn is_point(&self) -> bool {
		self.start.clone() + 1.into() == self.end
	}

	/// join two spans.
	///
	/// from the smallest start to the largest end, even disjoint ones.
	///
	/// # example
	/// ```
	/// assert_eq!(Span::join(&Span::new(1, 3), &Span::new(2, 4)), Span::new(1, 4));
	/// assert_eq!(Span::join(&Span::new(1, 3), &Span::new(1, 4)), Span::new(1, 4));
	/// assert_eq!(Span::join(&Span::new(1, 4), &Span::new(2, 3)), Span::new(1, 4));
	/// assert_eq!(Span::join(&Span::new(1, 1), &Span::new(3, 4)), Span::new(1, 4));
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
/// `span_around` takes a [`Matcher`] and returns a [`Matcher`] that matches by the matcher, propagating its error and capturing the span of the section it matched.
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

/// [`span_around`] matcher
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
/// `Cursor` is a `(value, offset)` tuple that provide an imperative matching experience, enhanced and integrated with the rest of gramex ecosystem.
///
/// for more info, read the [module documentation](crate::cursor).
///
/// # implementation guide
/// to implement `Cursor` for a custom type, the [`MatchAble`] must be specified, and the input and the offset must be provided though [`input`](Self::input), [`off`](Self::off) and [`off_mut`](Self::off_mut).
///
/// in addition, the `Cursor` [`Error`](Self::Error) type must be specified alongside its mapping from [`MatchError`] though [`map_error`](Self::map_error).
///
/// the `Cursor` has a `'src` lifetime used to split the input [slices](MatchAble::Slice) lifetime from `Cursor` inner mutations.
/// ```
/// struct MyCursor<'src> {
///     // input lives outside cursor for captures lifetimes independent from cursor mutation
///     value: &'src Tokens<'src>,
///     off: usize,
///     // support mutliple errors
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
/// and if you just need a simple cursor with unmodified [`MatchError`], check out [`SimpleCursor`].
pub trait Cursor<'src> {
	/// the [`MatchAble`] the `Cursor` works on.
	type MatchAble: MatchAble + ?Sized + 'src;
	/// the `Cursor` custom error type.
	type Error;

	/// return a reference to the [`MatchAble`] the `Cursor` works on.
	///
	/// this reference is unaffected by the `Cursor` mutations.
	fn input(&self) -> &'src Self::MatchAble;

	/// return the current offset.
	fn off(&self) -> usize;
	/// return a mutable reference to the offset.
	fn off_mut(&mut self) -> &mut usize;

	/// convert a [`MatchError`] to the `Cursor` [`Error`](Self::Error) type.
	fn map_error(&mut self, err: MatchError) -> Self::Error;

	/// get the [token](MatchAble::Token) at the current offset.
	///
	/// `peek` uses [`MatchAble::get_token`] under the hood.
	///
	/// it return `None` on end of input.
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

	/// get the [token](MatchAble::Token) at `n` tokens ahead of the current one.
	///
	/// `peek_next` uses [`MatchAble::skip_n`] under the hood.
	///
	/// it return `None` on end of input.
	///
	/// # example
	/// ```
	/// let mut cur = SimpleCursor::new("abc");
	/// assert_eq!(cur.peek_next(0), Some('a'));
	/// assert_eq!(cur.peek_next(1), Some('b'));
	/// assert_eq!(cur.peek_next(2), Some('c'));
	/// assert_eq!(cur.peek_next(3), None);
	///
	/// ```
	#[inline]
	fn peek_next(&self, n: usize) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		let input = self.input();
		let mut off = self.off();
		input.skip_n::<Test>(&mut off, n).ok()?;
		input.get_token(off)
	}

	/// check if the `Cursor` is at the end of the input.
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

	/// match by a [`Matcher`] at the current offset.
	///
	/// `eat` uses [`Parse`](crate::modes::Parse) [`Mode`] and the [`Cursor::Error`] instead of [`MatchError`].
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

	/// try match by a [`Matcher`] at the current offset.
	///
	/// `try_eat` uses [`Capture`](crate::modes::Capture) [`Mode`].
	///
	/// it fail atomicly, and return `None` on failure.
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

	/// match by a [`Matcher`] without advancing at the current offset.
	///
	/// `test` uses [`Test`] [`Mode`], and return `false` on failure.
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

	/// advance the `Cursor` by one token.
	///
	/// `skip` uses [`MatchAble::skip_n`] under the hood, and act as noop at end of input.
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
	/// the generated [mismatch](crate::result::MatchErrorKind::MisMatch) or [incomplete](crate::result::MatchErrorKind::InComplete) [`MatchError`] depending if the `Cursor` is at the end of the input, then it get mapped by [`Cursor::map_error`].
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
/// the `SimpleCursor` is the simplest [`Cursor`] possible, a real `(input, off)` tuple with regular [`MatchError`] and no extra bloat.
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
	/// create a new `SimpleCursor` wrapping the given input
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
