//! items and utilities used by the generated code

// re-export frequently used std items for shorter paths
pub use alloc_crate::vec::Vec;
pub use core::convert::{Infallible, Into};
pub use core::marker::PhantomData;
pub use core::option::Option;

pub use lean_string::LeanString;

pub use crate::cursor::Cursor;
pub use crate::result::{Expected, MatchError, MatchResult};

use crate::{MatchAble, Mode};
use lean_string::ToLeanString;

/// coerce the type into its `&MatchAble` form using method-call coercion
pub trait AsMatchAble {
	fn __as_matchable(&self) -> &Self {
		self
	}
}
impl<T: MatchAble + ?Sized> AsMatchAble for T {}

/// coerce the type into its `&mut Cursor` form using method-call coercion
pub trait AsCursor<T: ?Sized> {
	fn __as_cursor(&mut self) -> &mut Self {
		self
	}
}
impl<'src, T: MatchAble + ?Sized, C: Cursor<'src, MatchAble = T>> AsCursor<T> for C {}

/// produce an [`Expected`] for a negated unit
pub fn expected_not(expected: &Expected) -> Expected {
	match expected {
		Expected::None => Expected::None,
		_ => Expected::Not(expected.to_lean_string()),
	}
}
/// produce an [`Expected`] for an or expression
pub fn expected_or(cases: &[Expected]) -> Expected {
	let mut resolved = Vec::with_capacity(cases.len());
	for case in cases {
		if !matches!(case, Expected::None) {
			resolved.push(case.to_lean_string());
		}
	}
	if resolved.is_empty() { Expected::None } else { Expected::OneOf(resolved) }
}

/// like [`Result::unwrap`], but without the [`core::fmt::Debug`] bound
pub fn unwrap_result<T, E>(r: Result<T, E>) -> T {
	match r {
		Ok(v) => v,
		Err(_) => unreachable!(),
	}
}
/// a [`Mode`]-independent `Ok(())` that also resolves inference issues
pub fn ok_unit<M: Mode>() -> MatchResult<(), M> {
	Ok(M::wrap_success(()))
}
