use crate::{
	MatchAble, Mode,
	result::{Expected, MatchError, MatchResult},
};

#[doc(hidden)]
pub fn match_token<'src, M: Mode, T: MatchAble + ?Sized>(
	value: &'src T, off: &mut usize, pred: impl FnOnce(T::Token<'src>) -> Option<usize>,
	expected: impl FnOnce() -> Expected,
) -> MatchResult<T::Slice<'src>, M> {
	let Some(token) = value.get_token(*off) else {
		return M::err(|| MatchError::incomplete(expected(), *off));
	};
	if let Some(len) = pred(token) {
		*off += len;
		Ok(M::wrap_success(value.slice(*off - len..*off).unwrap()))
	} else {
		M::err(|| MatchError::mismatch(expected(), *off))
	}
}
#[doc(hidden)]
pub fn match_slice<'src, M: Mode, T: MatchAble + ?Sized, C>(
	value: &'src T, off: &mut usize, size: usize,
	pred: impl FnOnce(T::Slice<'src>) -> Option<C>, expected: impl FnOnce() -> Expected,
) -> MatchResult<C, M> {
	let Some(slice) = value.slice(*off..*off + size) else {
		return M::err(|| MatchError::incomplete(expected(), *off));
	};
	if let Some(res) = pred(slice) {
		*off += size;
		Ok(M::wrap_success(res))
	} else {
		M::err(|| MatchError::mismatch(expected(), *off))
	}
}

#[macro_export]
#[doc(hidden)]
#[rustfmt::skip]
macro_rules! define_slice_matcher {
	($(#for ($($bounds:tt)+))? $ty:ty, $matched:ty, 
		|$matcher:ident, $slice:ident| ($logic:expr, $expected:expr)
	) => {
		impl $(<$($bounds)+>)? $crate::Matcher<$matched> for $ty {
			type Capture<'src> = <$matched as $crate::MatchAble>::Slice<'src> 
				where $matched: 'src;
			fn do_match<'src, M: $crate::Mode>(
				&self, matched: &'src $matched, off: &mut usize,
			) -> $crate::result::MatchResult<Self::Capture<'src>, M> {
				let $matcher = self;
				#[allow(unused)]
				use $crate::MatchAble;
				$crate::derive::match_slice::<M, _, _>(matched, off, self.len(), 
					|$slice| $logic.then_some($slice), 
					|| <_ as $crate::Matcher<$matched>>::expected(&self)
				)
			}
			fn expected(&self) -> $crate::result::Expected {
				let $matcher = self;
				$expected
			}
		}
	};
}

#[rustfmt::skip]
#[macro_export]
#[doc(hidden)]
macro_rules! define_token_matcher {
	($(#for ($($bounds:tt)+))? $ty:ty, $matched:ty, 
		|$matcher:ident, $token:ident| ($logic:expr, $expected:expr)
	) => {
		impl $(<$($bounds)+>)? $crate::Matcher<$matched> for $ty {
			type Capture<'src> = <$matched as $crate::MatchAble>::Slice<'src> 
				where $matched: 'src;
			fn do_match<'src, M: $crate::Mode>(
				&self, matched: &'src $matched, off: &mut usize,
			) -> $crate::result::MatchResult<Self::Capture<'src>, M> {
				let $matcher = self;
				let expected = || <_ as $crate::Matcher<$matched>>::expected(&self);
				$crate::derive::match_token::<M, _>(
					matched, off, |$token| $logic, expected
				)
			}
			fn expected(&self) -> $crate::result::Expected {
				let $matcher = self;
				$expected
			}
		}
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! define_ref_matcher {
	($(#for ($($bounds:tt)+))? $ty:ty, for $matched:ty) => {
		impl $(<$($bounds)+>)? $crate::Matcher<$matched> for $ty {
			type Capture<'src> = <$matched as $crate::MatchAble>::Slice<'src>
				where $matched: 'src;
			fn do_match<'src, M: $crate::Mode>(
				&self, matched: &'src $matched, off: &mut usize,
			) -> $crate::result::MatchResult<Self::Capture<'src>, M> {
				::core::convert::AsRef::<$matched>::as_ref(self)
					.do_match::<M>(matched, off)
			}
			fn expected(&self) -> $crate::result::Expected {
				<_ as $crate::Matcher<$matched>>::expected(
					::core::convert::AsRef::<$matched>::as_ref(self)
				)
			}
		}
	};
}

#[doc(hidden)]
pub use {define_ref_matcher, define_slice_matcher, define_token_matcher};

#[cfg_attr(doc, doc(hidden))]
#[macro_export]
macro_rules! derive_slice_matchable {
	($box:ident(&[$item:ty]), false) => {
		$crate::derive::derive_slice_matchable!(#main $box(&[$item]));
	};
	($box:ident(&[$item:ty]), true) => {
		$crate::derive::derive_slice_matchable!(#main $box(&[$item]), true);
	};
	($box:ident(&[$item:ty]), WithRange) => {
		$crate::derive::derive_slice_matchable!(#main $box(&[$item]), true, true);
	};

	(#main $box:ident(&[$item:ty]) $(, $matchers_true:vis true $($range_true:vis, true)?)?) => {
		impl<'slice> $crate::MatchAble for $box<'slice> {
			type Slice<'src>
				= $box<'src>
			where
				Self: 'src;
			type Token<'src>
				= &'src $item
			where
				Self: 'src;

			fn len(&self) -> usize {
				self.0.len()
			}
			fn get_token<'src>(&'src self, off: usize) -> Option<&'src $item> {
				self.0.get(off)
			}
			fn slice<'src>(
				&'src self, range: ::core::ops::Range<usize>,
			) -> Option<Self::Slice<'src>> {
				self.0.get(range).map($box)
			}
		}

		$($matchers_true
			$crate::derive::define_slice_matcher!(#for('slice) $box<'_>, $box<'slice>, |matcher, slice| (
				slice.0 == matcher.0,
				$crate::result::Expected::None
			));
			$crate::derive::define_slice_matcher!(#for('slice) [$item], $box<'slice>, |matcher, slice| (
				slice.0 == matcher,
				$crate::result::Expected::None
			));
			$crate::derive::define_token_matcher!(#for('slice) $item, $box<'slice>, |matcher, token| (
				(token == matcher).then_some(1),
				$crate::result::Expected::None
			));
			$($range_true $crate::derive::define_token_matcher!(
				#for('slice) ::core::ops::RangeInclusive<$item>, $box<'slice>,
				|matcher, token| (
					matcher.contains(token).then_some(1),
					$crate::result::Expected::None
				)
			);)?
		)?
	};
}

#[doc(inline)]
pub use derive_slice_matchable;

#[cfg(feature = "macros")]
pub use gramex_macro::derive_enum_matcher;
