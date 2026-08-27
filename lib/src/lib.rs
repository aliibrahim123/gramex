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
	pub use alloc_crate::vec::Vec;
	pub use alloc_crate::{format, vec};

	pub fn unwrap_result<T, E>(r: Result<T, E>) -> T {
		match r {
			Ok(v) => v,
			Err(_) => unreachable!(),
		}
	}
}
