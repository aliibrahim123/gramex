#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(feature = "std")]
extern crate std as alloc_crate;

pub use gramex_macro::*;
mod core;
mod result;
pub mod str;
pub use core::{MatchAble, MatchFn, Matcher, Mode};
pub use result::{Expected, IntoResult, MatchError, MatchErrorKind, MatchResult};
pub mod modes {
	pub use crate::core::{Capture, Check, Parse, Test};
}
#[doc(hidden)]
pub mod __private {
	use alloc_crate::borrow::Cow;
	pub use alloc_crate::vec::Vec;
	pub use alloc_crate::{format, vec};
	pub use core::convert::{Infallible, Into};
	pub use core::marker::PhantomData;
	pub use core::option::Option;

	use crate::{Expected, MatchError};

	pub const EXPECTED_ANY: Expected = Expected::A(Cow::Borrowed("something"));
	pub fn error_any(off: usize) -> MatchError {
		MatchError::incomplete(EXPECTED_ANY, off)
	}
	pub fn expected_not(expected: Expected) -> Expected {
		Expected::A(format!("not {}", expected.value()).into())
	}
	pub fn error_not(expected: Expected, is_mismatch: bool, off: usize) -> MatchError {
		let expected = expected_not(expected);
		match is_mismatch {
			true => MatchError::mismatch(expected, off),
			false => MatchError::incomplete(expected, off),
		}
	}

	pub fn unwrap_result<T, E>(r: Result<T, E>) -> T {
		match r {
			Ok(v) => v,
			Err(_) => unreachable!(),
		}
	}
}
