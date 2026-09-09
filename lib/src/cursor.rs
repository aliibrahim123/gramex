use core::{
	cell::Cell,
	fmt::{self, Display, Formatter},
	ops::Add,
};

use crate::{
	MatchAble, Matcher,
	core::Test,
	result::{Expected, MatchError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleSpan<T = usize> {
	pub start: T,
	pub end: T,
}
impl<T: Clone + PartialEq + Ord + Add<usize, Output = T>> SimpleSpan<T> {
	fn new(start: T, end: T) -> Self {
		Self { start, end }
	}
	fn start(&self) -> T {
		self.start.clone()
	}
	fn end(&self) -> T {
		self.end.clone()
	}
	fn point(ind: T) -> Self {
		Self::new(ind.clone(), ind + 1)
	}
	fn is_point(&self) -> bool {
		self.start.clone() + 1 == self.end
	}
	fn join(&self, other: &Self) -> Self {
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

pub trait Cursor<'src> {
	type MatchAble: MatchAble + ?Sized + 'src;
	type Error;
	fn input(&self) -> &'src Self::MatchAble;
	fn off(&self) -> usize;
	fn off_mut(&mut self) -> &mut usize;
	fn map_error(&mut self, err: MatchError) -> Self::Error;

	fn peek(&'src self) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		self.input().get_token(self.off())
	}
	fn peek_next(&self, n: usize) -> Option<<Self::MatchAble as MatchAble>::Token<'src>> {
		let input = self.input();
		let mut off = self.off();
		input.skip_n::<Test>(&mut off, n).ok()?;
		input.get_token(off)
	}
	fn is_end(&self) -> bool {
		self.off() == self.input().len()
	}
	fn eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Result<M::Capture<'src>, Self::Error> {
		match matcher.parse(self.input(), self.off_mut()) {
			Ok(val) => Ok(val),
			Err(err) => Err(self.map_error(err)),
		}
	}
	fn try_eat<M: Matcher<Self::MatchAble>>(
		&mut self, matcher: M,
	) -> Option<M::Capture<'src>> {
		let mut off = self.off();
		matcher.capture(self.input(), &mut off).inspect(|_| *self.off_mut() = off)
	}
	fn test(&self, matcher: impl Matcher<Self::MatchAble>) -> bool {
		matcher.test(self.input(), &mut self.off())
	}
	fn skip(&mut self) {
		_ = self.input().skip_n::<Test>(self.off_mut(), 1);
	}
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
