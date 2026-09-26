//! macros for deriving core traits

use crate::{
	MatchAble, Mode,
	result::{Expected, MatchError, MatchResult},
};

/// `match_do` generification for mathcing by token
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

/// `match_do` generification for mathcing by slice
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

/// slice based `Matcher` template
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

/// token based `Matcher` template
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

/// `AsRef` based `Matcher` template
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

/// derive [`MatchAble`] for slice powered types.
///
/// ```gramex
/// let type = box:ident '(' '&' '[' token:type ']' ')';
/// let matchers = "matchers" ':' ("false" | "true" | "WithRange");
/// let expected = "expected" ':' ("None" | "Debug" | "Display");
/// let body = type ',' matchers (',' expected)?;
/// ```
///
/// `derive_slice_matchable` generate a [`MatchAble`] implementation for slice newtype structs with some core [`Matcher`](crate::Matcher).
///
/// slice newtype structs are any struct of kind `struct box<'src>(&'src [token])`.
///
/// the `type` argument is `box(&[token])` where `box` is the [`MatchAble`] and `token` is its [`Token`](MatchAble::Token), `item` is given a `'slice` lifetime linked to the tokens slice.
///
/// the generated [`MatchAble`] implementation has [`Slice`](MatchAble::Slice) of `box`, [`Token`](MatchAble::Token) of `&token`, slice indexes as the offsets, and other methods redirecting the inner slice ones.
///
/// the `matchers` argument can be:
/// - `false`: no extra [`Matcher`](crate::Matcher)s generated.
/// - `true`: generate [`Matcher`](crate::Matcher)s for `box`, `[token]` and `token`.
/// - `WithRange`: generate additional [`Matcher`](crate::Matcher) for [`RangeInclusive<token>`](core::ops::RangeInclusive).
///
/// the [`Matcher`](crate::Matcher)s capture the [`MatchAble::Slice`] they match.
///
/// the `expected` is optional part that specifies how [`Expected`] is created, it can be:
/// - `None`: [`Expected::None`].
/// - `Debug`: [`Expected::A`] of the matchers [`Debug`](core::fmt::Debug).
/// - `Display`: [`Expected::A`] of the matchers [`Display`](core::fmt::Display).
///
/// # example
/// ```
/// #[derive(Debug, PartialEq, PartialOrd)]
/// enum Token {
///     Nb(i64),
///     Add,
///     Sub,
///     Eq,
///     // ...
/// }
/// #[derive(Debug)]
/// struct Tokens<'src>(&'src [Token]);
/// derive_slice_matchable!(Tokens(&[Token]), matchers: true, expected: Debug);
///
/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
/// assert_eq!(tokens.len(), 3);
/// assert_eq!(tokens.get_token(1), Some(&Token::Add));
/// assert_eq!(tokens.slice(1..3), Some(Tokens(&[Token::Add, Token::Nb(2)])));
///
/// let mut off = 1;
/// tokens.skip_n::<Test>(&mut off, 2);
/// assert_eq!(off, 3);
///
/// assert_eq!(Token::Nb(1).parse(&tokens, &mut 0), Ok(Tokens(&[Token::Nb(1)])));
/// assert_eq!(
///     Token::Nb(2).parse(&tokens, &mut 0),
///     Err(MatchError::mismatch(Expected::A("Nb(2)".into()), 0))
/// );
///
/// assert!([Token::Nb(1), Token::Add].test(&tokens, &mut 0));
/// assert!(Tokens(&[Token::Nb(1), Token::Add]).test(&tokens, &mut 0));
/// ```
#[cfg_attr(doc, doc(hidden))]
#[macro_export]
#[cfg(doc)]
macro_rules! derive_slice_matchable {
	($box:ident(&[$item:ty]), matchers: false) => {};
	($box:ident(&[$item:ty]), matchers: $matchers:ident, expected: $expected:ident) => {};
}

#[cfg_attr(doc, doc(hidden))]
#[macro_export]
#[cfg(not(doc))]
macro_rules! derive_slice_matchable {
	($box:ident(&[$item:ty]), matchers: false) => {
		derive_slice_matchable!(#main $box(&[$item]), matchers: false, expected: None);
	};
	($box:ident(&[$item:ty]), matchers: $matchers:ident, expected: $expected:ident) => {
		derive_slice_matchable!(#main $box(&[$item]), matchers: $matchers, expected: $expected);
	};
	(#main
		$box:ident(&[$item:ty]),
	    $(matchers: false $matchers_false:vis)? $(matchers: true $matchers_true:vis)?
		$(matchers: WithRange $matchers_range:vis)?,
		expected: $expected:ident
	) => {
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
			derive_slice_matchable!(#derive_core_matchers $box(&[$item]), $expected);
		)?
		$($matchers_range
			derive_slice_matchable!(#derive_core_matchers $box(&[$item]), $expected);
			derive_slice_matchable!(#derive_range_matchers $box(&[$item]), $expected);
		)?
	};
	(#derive_core_matchers $box:ident(&[$item:ty]), $expected:ident) => {
		$crate::derive::define_slice_matcher!(#for('slice) $box<'_>, $box<'slice>,
			|matcher, slice| (
				slice.0 == matcher.0,
				derive_slice_matchable!(#expected: $expected, matcher)
			)
		);
		$crate::derive::define_slice_matcher!(#for('slice) [$item], $box<'slice>,
			|matcher, slice| (
				slice.0 == matcher,
				derive_slice_matchable!(#expected: $expected, matcher)
			)
		);
		$crate::derive::define_token_matcher!(#for('slice) $item, $box<'slice>,
			|matcher, token| (
				(token == matcher).then_some(1),
				derive_slice_matchable!(#expected: $expected, matcher)
			)
		);
	};
	(#derive_range_matchers $box:ident(&[$item:ty]), $expected:ident) => {
		$crate::derive::define_token_matcher!(
			#for('slice) ::core::ops::RangeInclusive<$item>, $box<'slice>,
			|matcher, token| (
				matcher.contains(token).then_some(1),
				derive_slice_matchable!(#expected_between: $expected, matcher)
			)
		);
	};
	(#expected: None, $matcher:ident) => {
		$crate::result::Expected::None
	};
	(#expected: $kind:ident, $matcher:ident) => {
		$crate::result::Expected::A(
			derive_slice_matchable!(#expected_block: $kind, $matcher)
		)
	};
	(#expected_between: None, $matcher:ident) => {
		$crate::result::Expected::None
	};
	(#expected_between: $kind:ident, $matcher:ident) => {
		$crate::result::Expected::Between(
			derive_slice_matchable!(#expected_block: $kind, $matcher.start()),
			derive_slice_matchable!(#expected_block: $kind, $matcher.end()),
		)
	};
	(#expected_block: Debug, $item:expr) => {{
		use core::fmt::Write;
		let mut buf = $crate::__private::LeanString::new();
		write!(buf, "{:?}", $item).unwrap();
		buf
	}};
	(#expected_block: Display, $item:expr) => {{
		use core::fmt::Write;
		let mut buf = $crate::__private::LeanString::new();
		write!(buf, "{}", $item).unwrap();
		buf
	}};
}

#[doc(inline)]
pub use derive_slice_matchable;

/// derive kind [`Matcher`](crate::Matcher) for token enum.
///
/// ```gramex
/// let field = "field" '=' rust_expr;
/// let expected = "expected" '=' expr_path;
/// let body = "for" matched_type:type (',' field)? (',' expected)?;
/// ```
///
/// the `derive_enum_matcher` is an attribute macro applied on enums that for each variant define a [`Matcher`](crate::Matcher) that matches one [token](MatchAble::Token) of that variant.
///
/// it take the [`MatchAble`] type, and define the [`Matcher`](crate::Matcher) as kebab case of the variant name.
///
/// if the variant is tuple, the [`Capture`](crate::Matcher::Capture) is a tuple of the variant fields, else it is the matched [`Token`](MatchAble::Token).
///
/// the `field` argument is an optional field path that specify where is the enum inside the token, if not specified, the enum is assumed to be the token.
///
/// the `expected` argument is an optional path that resolve to a `Fn(var: &str) -> Expected` called with the variant name to generate the [`Expected`], if not specified, [`Expected::None`] is used.
///
/// # example
/// ```
/// #[derive_enum_matcher(for Tokens<'slice>, field = .kind, expected = TokenKind::to_expected)]
/// #[derive(Debug, PartialEq)]
/// enum TokenKind<'src> {
///     Ident(&'src str),
///     Nb(Sign, u64),
///     Add,
///     Sub,
///     Eq,
///     // ...
/// }
/// impl TokenKind<'_> {
///     fn to_expected(var: &str) -> Expected {
///         Expected::A(var.to_lowercase().into())
///     }
/// }
/// #[derive(Debug, PartialEq)]
/// struct Token<'src> {
///     kind: TokenKind<'src>,
///     off: usize,
/// }
/// struct Tokens<'src>(&'src [Token<'src>]);
/// derive_slice_matchable!(Tokens(&[Token<'slice>]), matchers: false);
///
/// let tokens = Tokens(&[
///     Token { kind: TokenKind::Ident("abc"), off: 0 },
///     Token { kind: TokenKind::Add, off: 4 },
///     Token { kind: TokenKind::Nb(Sign::Pos, 123), off: 6 },
/// ]);
///
/// assert_eq!(
///     add.parse(&tokens, &mut 1),
///     Ok(&Token { kind: TokenKind::Add, off: 4 })
/// );
/// assert_eq!(
///     add.parse(&tokens, &mut 0),
///     Err(MatchError::mismatch("add".into(), 0))
/// );
/// assert_eq!(
///     add.parse(&tokens, &mut 3),
///     Err(MatchError::incomplete("add".into(), 3))
/// );
///
/// assert_eq!(ident.parse(&tokens, &mut 0), Ok(&"abc"));
/// assert_eq!(nb.parse(&tokens, &mut 2), Ok((&Sign::Pos, &123)));
/// ```
#[cfg(feature = "macros")]
#[doc(inline)]
pub use gramex_macro::derive_enum_matcher;
