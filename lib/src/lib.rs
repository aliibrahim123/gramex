#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(feature = "std")]
extern crate std as alloc_crate;

pub use gramex_macro::*;
#[cfg(feature = "bytes")]
pub mod bytes;
mod core;
pub mod general;
pub mod result;
#[cfg(feature = "str")]
pub mod str;
pub use core::{MatchAble, Matcher, Mode, check, matches, parse, try_match};
pub mod modes {
	pub use crate::core::{Capture, Check, Parse, Test};
}
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
