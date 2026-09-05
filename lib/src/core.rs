use core::{marker::PhantomData, ops::Range};

use crate::result::{Expected, IntoResult, MatchError, MatchResult};

pub trait MatchAble {
	type Token<'src>
	where
		Self: 'src;
	type Slice<'src>
	where
		Self: 'src;

	fn len(&self) -> usize;
	fn slice<'src>(&'src self, range: Range<usize>) -> Option<Self::Slice<'src>>;
	fn get_token<'src>(&'src self, off: usize) -> Option<Self::Token<'src>>;
	fn skip_n<M: Mode>(&self, off: &mut usize, n: usize) -> MatchResult<(), M> {
		let len = self.len();
		*off += n;
		if *off > len {
			*off = len;
			M::err(|| MatchError::incomplete(Expected::SomeThing, len))
		} else {
			Ok(M::wrap_success(()))
		}
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
}

impl<T: MatchAble + ?Sized, U: Matcher<T> + ?Sized> Matcher<T> for &U {
	type Capture<'src>
		= U::Capture<'src>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<U::Capture<'src>, M> {
		(*self).do_match::<M>(matched, off)
	}
	fn expected(&self) -> Expected {
		(*self).expected()
	}
}

impl<T: MatchAble + ?Sized> Matcher<T> for () {
	type Capture<'src>
		= ()
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, _matched: &'src T, _off: &mut usize,
	) -> MatchResult<(), M> {
		Ok(M::wrap_success(()))
	}
}

fn match_no_excess<'src, T: MatchAble + ?Sized, U: Matcher<T>, M: Mode>(
	value: &'src T, matcher: U,
) -> MatchResult<U::Capture<'src>, M> {
	let mut off = 0;
	let res = matcher.do_match::<M>(value, &mut off);
	if off == value.len() { res } else { M::err(|| MatchError::excess(off)) }
}
pub fn matches<T: MatchAble + ?Sized>(value: &T, matcher: impl Matcher<T>) -> bool {
	match_no_excess::<_, _, Test>(value, matcher).is_ok()
}
pub fn check<T: MatchAble + ?Sized>(
	value: &T, matcher: impl Matcher<T>,
) -> Result<(), MatchError> {
	match_no_excess::<_, _, Check>(value, matcher)
}
pub fn try_match<'src, T: MatchAble + ?Sized, U: Matcher<T>>(
	value: &'src T, matcher: U,
) -> Option<U::Capture<'src>> {
	match_no_excess::<_, _, Capture>(value, matcher).ok()
}
pub fn parse<'src, T: MatchAble + ?Sized, U: Matcher<T>>(
	value: &'src T, matcher: U,
) -> Result<U::Capture<'src>, MatchError> {
	match_no_excess::<_, _, Parse>(value, matcher)
}
