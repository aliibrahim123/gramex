use std::{fmt::Display, marker::PhantomData};

use crate::{Capturer, Expected, MatchAble, MatchError, MatchResult, Matcher};

#[macro_export]
#[doc(hidden)]
macro_rules! derive_check_from_test {
	($M:ty, $expected:expr) => {
		fn check(&self, matched: &$M, ind: &mut <$M>::Offset) -> MatchResult<(), $M> {
			match self.test(matched, ind) {
				true => Ok(()),
				false if *ind == matched.len() => Err(MatchError::incomplete($expected, *ind)),
				false => Err(MatchError::mismatch($expected, *ind)),
			}
		}
	};
}
#[macro_export]
#[doc(hidden)]
macro_rules! derive_matcher_from_capture {
	($M:ty) => {
		fn test(&self, matched: &$M, ind: &mut <$M>::Offset) -> bool {
			self.capture(matched, ind).is_ok()
		}
		fn check(&self, matched: &$M, ind: &mut <$M>::Offset) -> MatchResult<(), $M> {
			self.capture(matched, ind).map(|_| ())
		}
	};
}
#[macro_export]
#[doc(hidden)]
macro_rules! derive_slice_capture {
	($M:ty) => {
		type Capture<'a>
			= M::Slice<'a>
		where
			M: 'a;
		fn capture<'a>(&self, matched: &'a M, ind: &mut M::Offset) -> MatchResult<M::Slice<'a>, M> {
			let start_off = *ind;
			self.check(matched, ind).map(|_| matched.slice(start_off..*ind).unwrap())
		}
	};
}

#[doc(inline)]
pub use {derive_check_from_test, derive_matcher_from_capture, derive_slice_capture};

#[derive(Debug, Clone, Copy)]
pub struct TestFn<M: ?Sized, F> {
	fun: F,
	_marker: PhantomData<M>,
}
impl<M: MatchAble + ?Sized, F> TestFn<M, F>
where
	F: Fn(&M, &mut M::Offset) -> bool,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<M: MatchAble + ?Sized, F> Matcher<M> for TestFn<M, F>
where
	F: Fn(&M, &mut M::Offset) -> bool,
{
	fn test(&self, matched: &M, ind: &mut M::Offset) -> bool {
		(self.fun)(matched, ind)
	}
	derive_check_from_test!(M, Expected::None);
}

#[derive(Debug, Clone, Copy)]
pub struct CheckFn<M: ?Sized, F> {
	fun: F,
	_marker: PhantomData<M>,
}
impl<M: MatchAble + ?Sized, F> CheckFn<M, F>
where
	F: Fn(&M, &mut M::Offset) -> MatchResult<(), M>,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<M: MatchAble + ?Sized, F> Matcher<M> for CheckFn<M, F>
where
	F: Fn(&M, &mut M::Offset) -> MatchResult<(), M>,
{
	fn test(&self, matched: &M, ind: &mut <M as MatchAble>::Offset) -> bool {
		(self.fun)(matched, ind).is_ok()
	}
	fn check(&self, matched: &M, ind: &mut M::Offset) -> MatchResult<(), M> {
		(self.fun)(matched, ind)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureFn<M: ?Sized, F> {
	fun: F,
	_marker: PhantomData<M>,
}
impl<M: MatchAble + ?Sized, F> CaptureFn<M, F>
where
	F: for<'a> LifedCapture<'a, M>,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
#[doc(hidden)]
pub trait LifedCapture<'a, M: MatchAble + ?Sized + 'a> {
	type Capture: 'a;
	fn capture(&self, matched: &'a M, ind: &mut M::Offset) -> MatchResult<Self::Capture, M>;
}
impl<'a, M: MatchAble + ?Sized + 'a, C: 'a, F> LifedCapture<'a, M> for F
where
	F: Fn(&'a M, &mut M::Offset) -> MatchResult<C, M>,
{
	type Capture = C;
	fn capture(&self, matched: &'a M, ind: &mut M::Offset) -> MatchResult<C, M> {
		(self)(matched, ind)
	}
}
impl<M: MatchAble + ?Sized, F> Matcher<M> for CaptureFn<M, F>
where
	F: for<'a> LifedCapture<'a, M>,
{
	derive_matcher_from_capture!(M);
}
impl<M: MatchAble + ?Sized, F> Capturer<M> for CaptureFn<M, F>
where
	F: for<'a> LifedCapture<'a, M>,
{
	type Capture<'a>
		= <F as LifedCapture<'a, M>>::Capture
	where
		M: 'a;
	fn capture<'a>(
		&self, matched: &'a M, ind: &mut M::Offset,
	) -> MatchResult<Self::Capture<'a>, M> {
		self.fun.capture(matched, ind)
	}
}
#[derive(Debug, Clone, Copy)]
pub struct MatchSet<M: ?Sized, F1, F2, F3> {
	tester: F1,
	checker: F2,
	capturer: F3,
	_marker: PhantomData<M>,
}
impl<M: MatchAble + ?Sized, F1, F2, F3> MatchSet<M, F1, F2, F3>
where
	F1: Fn(&M, &mut M::Offset) -> bool,
	F2: Fn(&M, &mut M::Offset) -> MatchResult<(), M>,
	F3: for<'a> LifedCapture<'a, M>,
{
	pub fn new(tester: F1, checker: F2, capturer: F3) -> Self {
		Self { tester, checker, capturer, _marker: PhantomData }
	}
}
impl<M: MatchAble + ?Sized, F1, F2, F3> Matcher<M> for MatchSet<M, F1, F2, F3>
where
	F1: Fn(&M, &mut M::Offset) -> bool,
	F2: Fn(&M, &mut M::Offset) -> MatchResult<(), M>,
	F3: for<'a> LifedCapture<'a, M>,
{
	fn test(&self, matched: &M, ind: &mut M::Offset) -> bool {
		(self.tester)(matched, ind)
	}
	fn check(&self, matched: &M, ind: &mut M::Offset) -> MatchResult<(), M> {
		(self.checker)(matched, ind)
	}
}
impl<M: MatchAble + ?Sized, F1, F2, F3> Capturer<M> for MatchSet<M, F1, F2, F3>
where
	F1: Fn(&M, &mut M::Offset) -> bool,
	F2: Fn(&M, &mut M::Offset) -> MatchResult<(), M>,
	F3: for<'a> LifedCapture<'a, M>,
{
	type Capture<'a>
		= <F3 as LifedCapture<'a, M>>::Capture
	where
		M: 'a;
	fn capture<'a>(
		&self, matched: &'a M, ind: &mut <M as MatchAble>::Offset,
	) -> MatchResult<Self::Capture<'a>, M> {
		self.capturer.capture(matched, ind)
	}
}
