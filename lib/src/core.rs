use core::{marker::PhantomData, ops::Range};

use crate::{
	Expected, MatchResult,
	result::{IntoResult, MatchError},
};

pub trait MatchAble {
	type Slice<'a>
	where
		Self: 'a;

	fn len(&self) -> usize;
	fn slice<'a>(&'a self, range: Range<usize>) -> Option<Self::Slice<'a>>;
	fn skip_n(&self, off: &mut usize, n: usize) -> bool {
		let len = self.len();
		let overflowed = *off + n > len;
		*off = if overflowed { len } else { *off + n };
		!overflowed
	}
}

pub trait Mode {
	type Success<T>;
	type Error;

	const DO_CAPTURE: bool;
	const DO_ERROR: bool;

	type WithCapture: Mode;
	type WithError: Mode;
	type WithoutCapture: Mode;
	type WithoutError: Mode;

	fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self>;
	fn err<T>(err: impl FnOnce() -> MatchError) -> MatchResult<T, Self>;
}

macro_rules! decl_mod {
		($name:ident {
			$(capture: true $cap_true:vis)? $(capture: false $cap_false:vis)?,
			$(error: true $err_true:vis)? $(error: false $err_false:vis)?,
			+cap -> $plus_cap:ty,
			+err -> $plus_err:ty,
			-cap -> $minus_cap:ty,
			-err -> $minus_err:ty,
		}) => {
			pub struct $name;
			impl Mode for $name {
				$($cap_true type Success<T> = T;)?
				$($cap_false type Success<T> = ();)?
				$($err_true type Error = MatchError;)?
				$($err_false type Error = ();)?

				$($cap_true const DO_CAPTURE: bool = true;)?
				$($cap_false const DO_CAPTURE: bool = false;)?
				$($err_true const DO_ERROR: bool = true;)?
				$($err_false const DO_ERROR: bool = false;)?

				type WithCapture = $plus_cap;
				type WithError = $plus_err;
				type WithoutCapture = $minus_cap;
				type WithoutError = $minus_err;

				$($cap_true fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(cap())
				})?
				$($cap_false fn ok<T>(_cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(())
				})?
				$($err_true fn err<T>(err: impl FnOnce() -> MatchError) -> MatchResult<T, Self> {
					Err(err())
				})?
				$($err_false fn err<T>(_err: impl FnOnce() -> MatchError) -> MatchResult<T, Self> {
					Err(())
				})?
			}
		};
	}
decl_mod!(Test {
	capture: false, error: false,
	+cap -> Capture, +err -> Check,
	-cap -> Test, -err -> Test,
});
decl_mod!(Check {
	capture: false, error: true,
	+cap -> Parse, +err -> Check,
	-cap -> Check, -err -> Test,
});
decl_mod!(Capture {
	capture: true, error: false,
	+cap -> Capture, +err -> Parse,
	-cap -> Test, -err -> Capture,
});
decl_mod!(Parse {
	capture: true, error: true,
	+cap -> Parse, +err -> Parse,
	-cap -> Check, -err -> Capture,
});

pub trait Matcher<T: MatchAble + ?Sized> {
	type Capture<'a>
	where
		T: 'a;
	fn do_match<'a, M: Mode>(
		&self, matched: &'a T, off: &mut usize,
	) -> MatchResult<Self::Capture<'a>, M>;

	fn test(&self, matched: &T, off: &mut usize) -> bool {
		self.do_match::<Test>(matched, off).is_ok()
	}
	fn check(&self, matched: &T, off: &mut usize) -> Result<(), MatchError> {
		self.do_match::<Check>(matched, off)
	}
	fn capture<'a>(&self, matched: &'a T, off: &mut usize) -> Option<Self::Capture<'a>> {
		self.do_match::<Capture>(matched, off).ok()
	}
	fn parse<'a>(
		&self, matched: &'a T, off: &mut usize,
	) -> Result<Self::Capture<'a>, MatchError> {
		self.do_match::<Parse>(matched, off)
	}

	fn expected(&self) -> Expected {
		Expected::None
	}
}

#[doc(hidden)]
pub trait LifedMatchFn<'a, T: MatchAble + ?Sized + 'a> {
	type Capture: 'a;
	type Res: IntoResult<Output = Self::Capture>;
	fn call(&self, matched: &'a T, off: &mut usize) -> Self::Res;
}

impl<'a, T: MatchAble + ?Sized + 'a, R, F> LifedMatchFn<'a, T> for F
where
	F: Fn(&'a T, &mut usize) -> R,
	R: IntoResult,
	<R as IntoResult>::Output: 'a,
{
	type Capture = <R as IntoResult>::Output;
	type Res = R;
	fn call(&self, matched: &'a T, off: &mut usize) -> R {
		self(matched, off)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MatchFn<T: MatchAble + ?Sized, F> {
	fun: F,
	_marker: PhantomData<fn(&T)>,
}
impl<T: MatchAble + ?Sized, F> MatchFn<T, F>
where
	F: for<'a> LifedMatchFn<'a, T>,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<T: MatchAble + ?Sized, F> Matcher<T> for MatchFn<T, F>
where
	F: for<'a> LifedMatchFn<'a, T>,
{
	type Capture<'a>
		= <F as LifedMatchFn<'a, T>>::Capture
	where
		T: 'a;

	fn do_match<'a, M: Mode>(
		&self, matched: &'a T, off: &mut usize,
	) -> MatchResult<Self::Capture<'a>, M> {
		LifedMatchFn::call(&self.fun, matched, off).into_result::<M>(*off)
	}
}
