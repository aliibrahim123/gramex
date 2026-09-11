#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::match_bool)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::enum_glob_use)]

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(feature = "std")]
extern crate std as alloc_crate;

mod core;
pub mod cursor;
pub mod general;
pub mod result;

#[cfg(feature = "macros")]
pub use gramex_macro::{check, gramex, matcher, matches, parse, try_match};

#[cfg(feature = "bits")]
pub mod bits;
#[cfg(feature = "bytes")]
pub mod bytes;
#[cfg(feature = "str")]
pub mod str;

#[cfg(all(
	not(feature = "macros"),
	any(feature = "str", feature = "bytes", feature = "bits")
))]
pub(crate) mod derive;
#[cfg(feature = "macros")]
pub mod derive;

pub use core::{MatchAble, Matcher, Mode, check, matches, parse, try_match};
pub mod modes {
	pub use crate::core::{Capture, Check, Parse, Test};
}

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
