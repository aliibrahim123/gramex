use std::{fmt::Display, marker::PhantomData};

use crate::{Capturer, Expected, MatchAble, MatchError, MatchResult, Matcher};

#[macro_export]
#[doc(hidden)]
macro_rules! derive_check_from_test {
	($M:ty, |$_self:ident| $expected:expr) => {
		fn check(
			&self, matched: &$M, off: &mut <$M as $crate::MatchAble>::Offset,
		) -> $crate::MatchResult<(), $M> {
			let $_self = self;
			match self.test(matched, off) {
				true => Ok(()),
				false if *off == matched.len() => {
					Err($crate::MatchError::incomplete($expected, *off))
				}
				false => Err($crate::MatchError::mismatch($expected, *off)),
			}
		}
	};
	($M:ty, $expected:expr) => {
		fn check(
			&self, matched: &$M, off: &mut <$M as $crate::MatchAble>::Offset,
		) -> $crate::MatchResult<(), $M> {
			match self.test(matched, off) {
				true => Ok(()),
				false if *off == matched.len() => {
					Err($crate::MatchError::incomplete($expected, *off))
				}
				false => Err($crate::MatchError::mismatch($expected, *off)),
			}
		}
	};
}
#[macro_export]
#[doc(hidden)]
macro_rules! derive_matcher_from_capture {
	($M:ty) => {
		fn test(&self, matched: &$M, off: &mut <$M as $crate::MatchAble>::Offset) -> bool {
			self.capture(matched, off).is_ok()
		}
		fn check(
			&self, matched: &$M, off: &mut <$M as $crate::MatchAble>::Offset,
		) -> $crate::MatchResult<(), $M> {
			self.capture(matched, off).map(|_| ())
		}
	};
}
#[macro_export]
#[doc(hidden)]
macro_rules! derive_slice_capture {
	($M:ty) => {
		type Capture<'a>
			= <$M as $crate::MatchAble>::Slice<'a>
		where
			$M: 'a;
		fn capture<'a>(
			&self, matched: &'a $M, off: &mut <$M as MatchAble>::Offset,
		) -> $crate::MatchResult<<$M as $crate::MatchAble>::Slice<'a>, $M> {
			let start_off = *off;
			self.check(matched, off).map(|_| matched.slice(start_off..*off).unwrap())
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
	fn test(&self, matched: &M, off: &mut M::Offset) -> bool {
		(self.fun)(matched, off)
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
	fn test(&self, matched: &M, off: &mut <M as MatchAble>::Offset) -> bool {
		(self.fun)(matched, off).is_ok()
	}
	fn check(&self, matched: &M, off: &mut M::Offset) -> MatchResult<(), M> {
		(self.fun)(matched, off)
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
	fn capture(&self, matched: &'a M, off: &mut M::Offset) -> MatchResult<Self::Capture, M>;
}
impl<'a, M: MatchAble + ?Sized + 'a, C: 'a, F> LifedCapture<'a, M> for F
where
	F: Fn(&'a M, &mut M::Offset) -> MatchResult<C, M>,
{
	type Capture = C;
	fn capture(&self, matched: &'a M, off: &mut M::Offset) -> MatchResult<C, M> {
		(self)(matched, off)
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
		&self, matched: &'a M, off: &mut M::Offset,
	) -> MatchResult<Self::Capture<'a>, M> {
		self.fun.capture(matched, off)
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
	fn test(&self, matched: &M, off: &mut M::Offset) -> bool {
		(self.tester)(matched, off)
	}
	fn check(&self, matched: &M, off: &mut M::Offset) -> MatchResult<(), M> {
		(self.checker)(matched, off)
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
		&self, matched: &'a M, off: &mut <M as MatchAble>::Offset,
	) -> MatchResult<Self::Capture<'a>, M> {
		self.capturer.capture(matched, off)
	}
}
