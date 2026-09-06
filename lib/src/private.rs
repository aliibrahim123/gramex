pub use alloc_crate::vec::Vec;
pub use alloc_crate::{format, vec};
pub use core::convert::{Infallible, Into};
pub use core::marker::PhantomData;
pub use core::option::Option;
use lean_string::{LeanString, ToLeanString};

use crate::result::{Expected, MatchError, MatchResult};
use crate::{MatchAble, Mode};

pub trait AsMatchAble {
	fn __as_matchable(&self) -> &Self {
		self
	}
}
impl<T: MatchAble + ?Sized> AsMatchAble for T {}

pub fn error_any(off: usize) -> MatchError {
	MatchError::incomplete(Expected::SomeThing, off)
}
pub fn expected_not(expected: Expected) -> Expected {
	match expected {
		Expected::None => Expected::None,
		_ => Expected::Not(expected.to_lean_string()),
	}
}
pub fn error_not(expected: Expected, is_mismatch: bool, off: usize) -> MatchError {
	let expected = expected_not(expected);
	match is_mismatch {
		true => MatchError::mismatch(expected, off),
		false => MatchError::incomplete(expected, off),
	}
}
pub fn expected_or(cases: &[Expected]) -> Expected {
	let mut resolved = Vec::with_capacity(cases.len());
	for case in cases {
		if !matches!(case, Expected::None) {
			resolved.push(case.to_lean_string());
		}
	}
	if resolved.len() > 0 { Expected::OneOf(resolved) } else { Expected::None }
}
pub fn error_or(cases: &[Expected], is_mismatch: bool, off: usize) -> MatchError {
	let expected = expected_or(cases);
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
pub fn ok_unit<M: Mode>() -> MatchResult<(), M> {
	Ok(M::wrap_success(()))
}
