use core::{
	fmt::{self, Display, Formatter},
	ops::Add,
};

use crate::{
	MatchAble, Matcher, Mode,
	core::Test,
	result::{Expected, MatchError, MatchResult},
};

#[cfg(feature = "macros")]
pub use gramex_macro::{eat, match_map, test, try_eat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleSpan<T = usize> {
	pub start: T,
	pub end: T,
}
impl<T: Clone + PartialEq + Ord + Add<Output = T> + From<u8>> SimpleSpan<T> {
	pub fn new(start: T, end: T) -> Self {
		Self { start, end }
	}
	pub fn start(&self) -> T {
		self.start.clone()
	}
	pub fn end(&self) -> T {
		self.end.clone()
	}
	pub fn point(ind: T) -> Self {
		Self::new(ind.clone(), ind + 1.into())
	}
	pub fn is_point(&self) -> bool {
		self.start.clone() + 1.into() == self.end
	}
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

pub fn span_around<T: MatchAble + ?Sized, M: Matcher<T>>(matcher: M) -> SpanAround<M> {
	SpanAround(matcher)
}
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
pub trait Cursor<'src> {
	type MatchAble: MatchAble + ?Sized + 'src;
	type Error;
	fn input(&self) -> &'src Self::MatchAble;
	fn off(&self) -> usize;
	fn off_mut(&mut self) -> &mut usize;
	fn map_error(&mut self, err: MatchError) -> Self::Error;

	#[inline]
	fn peek(&'src self) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		self.input().get_token(self.off())
	}
	#[inline]
	fn peek_next(&self, n: usize) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		let input = self.input();
		let mut off = self.off();
		input.skip_n::<Test>(&mut off, n).ok()?;
		input.get_token(off)
	}
	#[inline]
	fn is_end(&self) -> bool {
		self.off() == self.input().len()
	}
	#[inline]
	fn eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Result<M::Capture<'src>, Self::Error> {
		matcher.parse(self.input(), self.off_mut()).map_err(|err| self.map_error(err))
	}
	#[inline]
	fn try_eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Option<M::Capture<'src>> {
		let mut off = self.off();
		matcher.capture(self.input(), &mut off).inspect(|_| *self.off_mut() = off)
	}
	#[inline]
	fn test(&self, matcher: impl Matcher<Self::MatchAble>) -> bool {
		matcher.test(self.input(), &mut self.off())
	}
	#[inline]
	fn skip(&mut self) {
		_ = self.input().skip_n::<Test>(self.off_mut(), 1);
	}
	#[inline]
	fn rewind(&mut self, off: usize) {
		*self.off_mut() = off;
	}
	fn expected(&mut self, expected: impl Into<Expected>) -> Result<(), Self::Error> {
		let err = match self.is_end() {
			true => MatchError::incomplete(expected.into(), self.off()),
			false => MatchError::mismatch(expected.into(), self.off()),
		};
		Err(self.map_error(err))
	}
}
