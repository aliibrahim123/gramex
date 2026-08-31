use core::{marker::PhantomData, ops::Range};

use crate::{
	Expected, MatchResult,
	result::{IntoResult, MatchError},
};

pub trait MatchAble {
	type Slice<'src>
	where
		Self: 'src;

	fn len(&self) -> usize;
	fn slice<'src>(&'src self, range: Range<usize>) -> Option<Self::Slice<'src>>;
	fn skip_n(&self, off: &mut usize, n: usize) -> bool {
		let len = self.len();
		let overflowed = *off + n > len;
		*off = if overflowed { len } else { *off + n };
		!overflowed
	}

	#[doc(hidden)]
	fn __len(&self) -> usize {
		self.len()
	}
	#[doc(hidden)]
	fn __slice<'src>(&'src self, range: Range<usize>) -> Option<Self::Slice<'src>> {
		self.slice(range)
	}
	#[doc(hidden)]
	fn __skip_n(&self, off: &mut usize, n: usize) -> bool {
		self.skip_n(off, n)
	}
}

pub trait Mode {
	type Success<T>;
	type Error;

	const DO_CAPTURE: bool;
	const DO_ERROR: bool;

	type WithCapture: Mode<Error = Self::Error>;
	type WithError: Mode;
	type WithoutCapture: Mode<Error = Self::Error>;
	type WithoutError: Mode<Error = ()>;

	fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self>;
	fn err<T>(err: impl FnOnce() -> MatchError) -> Result<T, Self::Error>;

	fn wrap_success<T>(val: T) -> Self::Success<T>;
	fn wrap_error(err: MatchError) -> Self::Error;

	#[inline]
	fn unwrap_success<T>(val: Self::Success<T>) -> T {
		let _ = val;
		panic!("unwrap_success called on no-capture mode")
	}
	#[inline]
	fn unwrap_error(val: Self::Error) -> MatchError {
		let _ = val;
		panic!("unwrap_error called on no-error mode")
	}
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

				#[inline]
				$($cap_true fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(cap())
				})?
				$($cap_false fn ok<T>(_cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(())
				})?
				#[inline]
				$($err_true fn err<T>(err: impl FnOnce() -> MatchError) -> Result<T, Self::Error> {
					Err(err())
				})?
				$($err_false fn err<T>(_err: impl FnOnce() -> MatchError) -> Result<T, Self::Error> {
					Err(())
				})?

				#[inline]
				$($cap_true fn wrap_success<T>(val: T) -> Self::Success<T> { val })?
				$($cap_false fn wrap_success<T>(_val: T) -> Self::Success<T> { () })?
				#[inline]
				$($err_true fn wrap_error(err: MatchError) -> Self::Error { err })?
				$($err_false fn wrap_error(_err: MatchError) -> Self::Error { () })?


				$(#[inline] $cap_true fn unwrap_success<T>(val: Self::Success<T>) -> T { val })?
				$(#[inline] $err_true fn unwrap_error(val: Self::Error) -> MatchError { val })?
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
	type Capture<'src>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M>;

	#[inline]
	fn test(&self, matched: &T, off: &mut usize) -> bool {
		self.do_match::<Test>(matched, off).is_ok()
	}
	#[inline]
	fn check(&self, matched: &T, off: &mut usize) -> Result<(), MatchError> {
		self.do_match::<Check>(matched, off)
	}
	#[inline]
	fn capture<'src>(
		&self, matched: &'src T, off: &mut usize,
	) -> Option<Self::Capture<'src>> {
		self.do_match::<Capture>(matched, off).ok()
	}
	#[inline]
	fn parse<'src>(
		&self, matched: &'src T, off: &mut usize,
	) -> Result<Self::Capture<'src>, MatchError> {
		self.do_match::<Parse>(matched, off)
	}

	fn expected(&self) -> Expected {
		Expected::None
	}

	#[inline]
	#[doc(hidden)]
	fn __do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		self.do_match::<M>(matched, off)
	}

	#[doc(hidden)]
	fn __expected(&self) -> Expected {
		self.expected()
	}
}

#[doc(hidden)]
pub trait LifedMatchFn<'src, T: MatchAble + ?Sized + 'src> {
	type Capture: 'src;
	type Res: IntoResult<Output = Self::Capture>;
	fn call(&self, matched: &'src T, off: &mut usize) -> Self::Res;
}

impl<'src, T: MatchAble + ?Sized + 'src, R, F> LifedMatchFn<'src, T> for F
where
	F: Fn(&'src T, &mut usize) -> R,
	R: IntoResult,
	<R as IntoResult>::Output: 'src,
{
	type Capture = <R as IntoResult>::Output;
	type Res = R;

	fn call(&self, matched: &'src T, off: &mut usize) -> R {
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
	F: for<'src> LifedMatchFn<'src, T>,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<T: MatchAble + ?Sized, F> Matcher<T> for MatchFn<T, F>
where
	F: for<'src> LifedMatchFn<'src, T>,
{
	type Capture<'src>
		= <F as LifedMatchFn<'src, T>>::Capture
	where
		T: 'src;

	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		LifedMatchFn::call(&self.fun, matched, off).into_result::<M>(*off)
	}
}
