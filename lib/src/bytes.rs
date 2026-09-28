//! [`[u8]`](prim@slice) byte slices matching implementation.
//!
//! when `bytes` feature is enabled, [`[u8]`](prim@slice) matching gets enabled with additional [`Matcher`](crate::Matcher)s through this module.
//!
//! # `[u8]` [`MatchAble`] implementation
//!
//! [`MatchAble`] is implemented for `[u8]` where [`Slice`](MatchAble::Slice) is [`&[u8]`](prim@slice), [`Token`](MatchAble::Token) is [`u8`] and offsets are indexes of `[u8]` slice.
//!
//! ```
//! let bytes: &[u8] = &[1, 2, 3];
//! assert_eq!(MatchAble::len(bytes), 3);
//!
//! assert_eq!(MatchAble::slice(bytes, 1..3), Some(&[2, 3][..]));
//! assert_eq!(MatchAble::slice(bytes, 1..4), None);
//! assert_eq!(MatchAble::get_token(bytes, 2), Some(3));
//! assert_eq!(MatchAble::get_token(bytes, 3), None);
//!
//! let mut off = 0;
//! assert!(MatchAble::skip_n::<Test>(bytes, &mut off, 2).is_ok());
//! assert_eq!(off, 2);
//! ```
//!
//! # core [`Matcher`](crate::Matcher)s
//!
//! the core [`Matcher`](crate::Matcher)s for [`[u8]`](prim@slice) are: `[u8]`, [`u8`], [`RangeInclusive<u8>`] for range matching, [`Vec<u8>`], and [`[u8; N]`](prim@array).
//!
//! in addition to `&[u8]`, [`Box<[u8]>`](Box), [`Rc<[u8]>`](alloc_crate::rc::Rc) and [`Arc<[u8]>`](alloc_crate::sync::Arc) that are common to all [`MatchAble`]s.
//!
//! these [`Matcher`](crate::Matcher)s match with the `[u8]` slice they match.
//!
//! ```
//! assert_eq!(parse(&[1, 2, 3][..], &[1, 2, 3]), Ok(&[1, 2, 3][..]));
//! assert_eq!(
//!     parse(&[1, 2, 4][..], &[1, 2, 3]),
//!     Err(MatchError::mismatch("`01 02 03`".into(), 0))
//! );
//! assert_eq!(
//!     parse(&[1, 2][..], &[1, 2, 3]),
//!     Err(MatchError::incomplete("`01 02 03`".into(), 0))
//! );
//!
//! assert!(matches(&[1][..], 1));
//! assert!(matches(&[1][..], 1..=2));
//!
//! assert!(matches(&[1, 2, 3][..], vec![1, 2, 3]));
//! assert!(matches(&[1, 2, 3][..], [1, 2, 3]));
//! assert!(matches(&[1, 2, 3][..], Box::new([1, 2, 3])));
//! ```
//!
//! # extra [`Matcher`](crate::Matcher)s
//! the `bytes` module exports multiple [`Matcher`](crate::Matcher)s that match and capture bytes of common kinds and patterns: [`u32_le`], [`be`], [`a_le`]...
//!
//! ```
//! assert_eq!(try_match(&[1, 2, 3, 4][..], be(0x1020304u32)), Some(&[1, 2, 3, 4][..]));
//! assert_eq!(try_match(&[1, 2][..], i16_le), Some(0x201));
//! assert_eq!(
//!     try_match(&[1,2,3,4, 5,6,7,8][..], a_le(|n: u64| n > 0)),
//!     Some(0x8070605_04030201)
//! );
//! ```

use core::{
	fmt::Write,
	marker::PhantomData,
	mem::size_of,
	ops::{Range, RangeInclusive},
};

use lean_string::LeanString;

use crate::{
	MatchAble,
	bytes::matchers::{ABE, ALE, Aligned, BE, LE},
	derive::{define_ref_matcher, define_slice_matcher, define_token_matcher},
	result::Expected,
};

impl MatchAble for [u8] {
	type Token<'src> = u8;
	type Slice<'src> = &'src [u8];

	#[inline]
	fn len(&self) -> usize {
		self.len()
	}
	#[inline]
	fn get_token<'src>(&'src self, off: usize) -> Option<u8> {
		self.get(off).copied()
	}
	#[inline]
	fn slice<'src>(&'src self, range: Range<usize>) -> Option<&'src [u8]> {
		self.get(range)
	}
}

define_token_matcher!(u8, [u8], |matcher, byte| (
	(byte == *matcher).then_some(1),
	Expected::A(to_hex(&[*matcher]))
));
define_slice_matcher!([u8], [u8], |matcher, slice| (
	slice == matcher,
	Expected::A(to_hex(matcher))
));
define_token_matcher!(RangeInclusive<u8>, [u8], |matcher, byte| (
	matcher.contains(&byte).then_some(1),
	Expected::Between(to_hex(&[*matcher.start()]), to_hex(&[*matcher.end()]))
));

/// convert [1, 2, 3] -> `01 02 03`
fn to_hex(slice: &[u8]) -> LeanString {
	let mut str = LeanString::from('`');
	for (ind, byte) in slice.iter().enumerate() {
		if ind > 0 {
			str.push(' ');
		}
		write!(str, "{byte:02x}").unwrap();
	}
	str.push('`');
	str
}

define_ref_matcher!(Vec<u8>, for [u8]);
define_ref_matcher!(#for(const N: usize) [u8; N], for [u8]);

/// test if current offset is `align` bytes aligned.
///
/// `aligned` produces a [`Matcher`](crate::Matcher) that matches with `()` if the current offset is aligned to an `align`-byte boundary, otherwise it fails.
///
/// # example
/// ```
/// let off = 0;
/// assert!(aligned(2).test(&[1, 2, 3, 4][..], &mut off));
/// assert_eq!(off, 0);
/// assert!(!aligned(2).test(&[1, 2, 3, 4][..], &mut 3))
/// ```
pub fn aligned(align: usize) -> Aligned {
	Aligned { align }
}

/// types convertible to little endian byte arrays.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
pub trait AsLEBytes: Sized + Clone {
	/// the number of bytes to represent this type
	const BYTES: usize;

	/// a  `[u8; N]` byte array representing this type.
	type Bytes: AsRef<[u8]>;

	/// convert a little endian byte array to this type.
	///
	/// `bytes` is guaranteed to be [`BYTES`](Self::BYTES) sized.
	fn from_bytes(bytes: &[u8]) -> Self;

	/// convert this type to a little endian byte array.
	fn to_bytes(&self) -> Self::Bytes;
}

/// types convertible to big endian byte arrays.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
pub trait AsBEBytes: Sized + Clone {
	/// the number of bytes to represent this type
	const BYTES: usize;

	/// a  `[u8; N]` byte array representing this type.
	type Bytes: AsRef<[u8]> + Copy;

	/// convert a big endian byte array to this type.
	fn from_bytes(bytes: &[u8]) -> Self;

	/// convert this type to a big endian byte array.
	fn to_bytes(&self) -> Self::Bytes;
}

/// matches by the little endian byte view of `value`.
///
/// `le` produces a [`Matcher`](crate::Matcher) that converts `value` into a little endian byte array and matches with it, capturing the matched `[u8]` slice.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
///
/// # example
/// ```
/// assert_eq!(try_match(&[1, 2, 3, 0][..], le(0x30201u32)), Some(&[1, 2, 3, 0][..]));
/// assert!(!matches(&[1, 2, 3, 4][..], le(0x30201u32)));
/// assert!(!matches(&[1, 2, 3][..], le(0x30201u32)));
/// assert!(matches(&[255, 255][..], le(-1i16)));
/// ```
pub fn le<T: AsLEBytes>(value: T) -> LE<T> {
	LE(value)
}

/// matches by the big endian byte view of `value`.
///
/// `be` produces a [`Matcher`](crate::Matcher) that converts `value` into a big endian byte array and matches with it, capturing the matched `[u8]` slice.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
///
/// # example
/// ```
/// assert_eq!(try_match(&[0, 1, 2, 3][..], be(0x10203u32)), Some(&[0, 1, 2, 3][..]));
/// assert!(!matches(&[0, 1, 2, 4][..], be(0x10203u32)));
/// assert!(!matches(&[1, 2, 3][..], be(0x10203u32)));
/// assert!(matches(&[255, 255][..], be(-1i16)));
/// ```
pub fn be<T: AsBEBytes>(value: T) -> BE<T> {
	BE(value)
}

/// matches a little endian encoded value by a predicate.
///
/// `a_le` produces a [`Matcher`](crate::Matcher) that decodes a little endian byte slice as a `T` value and tests it by `pred`. If `pred` returns `true` it matches with the value, else it fails.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
///
/// # example
/// ```
/// let gt_100 = a_le(|v: u32| v > 100);
/// assert_eq!(try_match(&[1, 2, 3, 0][..], gt_100), Some(0x30201));
/// assert!(!matches(&[1, 0, 0, 0][..], gt_100));
/// assert!(!matches(&[1, 2, 3][..], gt_100));
/// assert!(matches(&[255, 255][..], a_le(|v: i16| v < 0)));
/// ```
pub fn a_le<T: AsLEBytes, F: Fn(T) -> bool>(pred: F) -> ALE<T, F> {
	ALE(pred, PhantomData)
}

/// matches a big endian encoded value by a predicate.
///
/// `a_be` produces a [`Matcher`](crate::Matcher) that decodes a big endian byte slice as a `T` value and tests it by `pred`. If `pred` returns `true` it matches with the value, else it fails.
///
/// it is implemented for:
/// - [`u16`], [`u32`], [`u64`], [`u128`], [`usize`],
/// - [`i16`], [`i32`], [`i64`], [`i128`], [`isize`],
/// - [`f32`], [`f64`]
///
/// # example
/// ```
/// let gt_100 = a_be(|v: u32| v > 100);
/// assert_eq!(try_match(&[0, 1, 2, 3][..], gt_100), Some(0x10203));
/// assert!(!matches(&[0, 0, 0, 1][..], gt_100));
/// assert!(!matches(&[1, 2, 3][..], gt_100));
/// assert!(matches(&[255, 255][..], a_be(|v: i16| v < 0)));
/// ```
pub fn a_be<T: AsBEBytes, F: Fn(T) -> bool>(pred: F) -> ABE<T, F> {
	ABE(pred, PhantomData)
}

macro_rules! define_nb_matcher {
    ($(($ty:ident, $le:ident, $be:ident)),+ $(,)?) => {
        $(
            define_nb_matcher!(#impl_bytes $ty, AsLEBytes, from_le_bytes, to_le_bytes);
            define_nb_matcher!(#impl_bytes $ty, AsBEBytes, from_be_bytes, to_be_bytes);

            define_nb_matcher!(#matcher $ty, "little", VLE, $le, to_le_bytes);
            define_nb_matcher!(#matcher $ty, "big", VBE, $be, to_be_bytes);
        )+
    };

    (#impl_bytes $ty:ident, $Trait:ident, $from_bytes:ident, $to_bytes:ident) => {
        impl $Trait for $ty {
            const BYTES: usize = size_of::<$ty>();
            type Bytes = [u8; size_of::<$ty>()];
            fn from_bytes(bytes: &[u8]) -> Self {
                <$ty>::$from_bytes(bytes.try_into().unwrap())
            }
            fn to_bytes(&self) -> Self::Bytes {
                self.$to_bytes()
            }
        }
    };

    (#matcher $ty:ident, $endian:literal, $VXE:ident, $xe:ident, $to_bytes:ident) => {
        #[doc = concat!(
            "matches with a ", $endian, " endian encoded [`", stringify!($ty), "`] value.\n\n",
            "`", stringify!($xe), "` is a [`Matcher`](crate::Matcher) that matches ",
            "a byte slice and captures its ", $endian, " endian decoded `", stringify!($ty), "` value.\n\n",
            "# example\n\n```\n",
            "let value = 0x123 as ", stringify!($ty), ";\n",
            "assert_eq!(try_match(&value.", stringify!($to_bytes), "()[..], ", stringify!($xe), "), Some(value));\n",
            "assert!(!matches(&[1][..], ", stringify!($xe), "));\n",
            "```"
        )]
        #[allow(nonstandard_style)]
        pub const $xe: matchers::$VXE<$ty> = matchers::$VXE(PhantomData);
    };
}

define_nb_matcher![
	(u16, u16_le, u16_be),
	(u32, u32_le, u32_be),
	(u64, u64_le, u64_be),
	(u128, u128_le, u128_be),
	(i16, i16_le, i16_be),
	(i32, i32_le, i32_be),
	(i64, i64_le, i64_be),
	(i128, i128_le, i128_be),
	(usize, usize_le, usize_be),
	(isize, isize_le, isize_be),
	(f32, f32_le, f32_be),
	(f64, f64_le, f64_be),
];

/// [bytes](crate::bytes) items [`Matcher`](crate::Matcher)s
pub mod matchers {
	use core::{any::type_name, fmt::Write, marker::PhantomData};
	use lean_string::LeanString;

	use crate::{
		Matcher, Mode,
		bytes::{AsBEBytes, AsLEBytes, to_hex},
		derive::match_slice,
		result::{Expected, MatchError, MatchResult},
	};

	fn expected_t_value(ty: &'static str) -> Expected {
		let mut str = LeanString::new();
		write!(str, "a {ty} value").unwrap();
		Expected::A(str)
	}

	/// [`aligned`](super::aligned) [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct Aligned {
		pub(crate) align: usize,
	}

	impl Matcher<[u8]> for Aligned {
		type Capture<'src> = ();
		#[inline]
		fn do_match<'src, M: Mode>(
			&self, _matched: &'src [u8], off: &mut usize,
		) -> MatchResult<(), M> {
			if *off % self.align == 0 {
				Ok(M::wrap_success(()))
			} else {
				M::err(|| MatchError::mismatch(self.expected(), *off))
			}
		}
		fn expected(&self) -> Expected {
			let mut str = LeanString::new();
			write!(str, "{}-byte alignment", self.align).unwrap();
			Expected::A(str)
		}
	}

	/// [`le`](super::le) [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct LE<T>(pub(crate) T);

	impl<T: AsLEBytes> Matcher<[u8]> for LE<T> {
		type Capture<'src> = &'src [u8];
		#[inline]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<&'src [u8], M> {
			self.0.to_bytes().as_ref().do_match::<M>(matched, off)
		}
		fn expected(&self) -> Expected {
			Expected::A(to_hex(self.0.to_bytes().as_ref()))
		}
	}

	/// [`be`](super::be) [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct BE<T>(pub(crate) T);

	impl<T: AsBEBytes> Matcher<[u8]> for BE<T> {
		type Capture<'src> = &'src [u8];
		#[inline]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<&'src [u8], M> {
			self.0.to_bytes().as_ref().do_match::<M>(matched, off)
		}
		fn expected(&self) -> Expected {
			Expected::A(to_hex(self.0.to_bytes().as_ref()))
		}
	}

	/// `tn_le` [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct VLE<T>(pub(crate) PhantomData<T>);

	impl<T: AsLEBytes> Matcher<[u8]> for VLE<T> {
		type Capture<'src> = T;
		#[rustfmt::skip]
		#[inline]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<T, M> {
			match_slice::<M, _, _>(matched, off, T::BYTES, |bytes| {
				Some(T::from_bytes(bytes))
			}, || self.expected())
		}
		fn expected(&self) -> Expected {
			expected_t_value(type_name::<T>())
		}
	}

	/// `tn_be` [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct VBE<T>(pub(crate) PhantomData<T>);

	impl<T: AsBEBytes> Matcher<[u8]> for VBE<T> {
		type Capture<'src> = T;
		#[rustfmt::skip]
		#[inline]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<T, M> {
			match_slice::<M, _, _>(matched, off, T::BYTES, |bytes| {
				Some(T::from_bytes(bytes))
			}, || self.expected())
		}
		fn expected(&self) -> Expected {
			expected_t_value(type_name::<T>())
		}
	}

	/// [`a_le`](super::a_le) [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct ALE<T, F: Fn(T) -> bool>(pub(crate) F, pub(crate) PhantomData<T>);

	impl<T: AsLEBytes, F: Fn(T) -> bool> Matcher<[u8]> for ALE<T, F> {
		type Capture<'src> = T;
		#[inline]
		#[rustfmt::skip]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<T, M> {
			match_slice::<M, _, _>(matched, off, T::BYTES, |bytes| {
				let value = T::from_bytes(bytes);
				self.0(value.clone()).then_some(value)
			}, || self.expected())
		}
		fn expected(&self) -> Expected {
			expected_t_value(type_name::<T>())
		}
	}

	/// [`a_be`](super::a_be) [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct ABE<T, F: Fn(T) -> bool>(pub(crate) F, pub(crate) PhantomData<T>);

	impl<T: AsBEBytes, F: Fn(T) -> bool> Matcher<[u8]> for ABE<T, F> {
		type Capture<'src> = T;
		#[inline]
		#[rustfmt::skip]
		fn do_match<'src, M: Mode>(
			&self, matched: &'src [u8], off: &mut usize,
		) -> MatchResult<T, M> {
			match_slice::<M, _, _>(matched, off, T::BYTES, |bytes| {
				let value = T::from_bytes(bytes);
				self.0(value.clone()).then_some(value)
			}, || self.expected())
		}
		fn expected(&self) -> Expected {
			expected_t_value(type_name::<T>())
		}
	}

	#[cfg(feature = "bits")]
	pub use super::bits_ext::{WordBE, WordLE};
}

#[cfg(feature = "bits")]
mod bits_ext {
	use core::{fmt::Write, marker::PhantomData};
	use lean_string::LeanString;

	use crate::{
		MatchAble, Matcher, Mode,
		bits::{BBits, Bits, LBits},
		result::{MatchError, MatchResult},
	};

	pub trait BitsType: MatchAble {}
	impl BitsType for LBits {}
	impl BitsType for BBits {}

	/// matches a little endian `bytes` word by a bits [`Matcher`].
	///
	/// `word_le` takes a [`LBits`]/[`BBits`] `matcher` and produces a [`Matcher`] that matches a little endian byte section of `bytes` length, then matches its bits in little / big endian order with `matcher`, propagating its result.
	///
	/// # example
	/// ```
	/// let bit_matcher = matcher!(for LBits, (a = _[4]) {b(8, 0xaa)} (b = _[4]));
	/// assert_eq!(
	///     try_match(&[0xa1, 0x2a][..], word_le(2, bit_matcher)),
	///     Some((LBits::new(4, 1), LBits::new(4, 2)))
	/// );
	/// assert!(!matches(&[0xb1, 0x2a][..], word_le(2, bit_matcher)));
	/// assert!(!matches(&[0xa1][..], word_le(2, bit_matcher)));
	/// ```
	pub fn word_le<T: BitsType, M: Matcher<T>>(bytes: u8, matcher: M) -> WordLE<T, M> {
		assert!(bytes <= 8 && bytes != 0);
		WordLE { bytes, matcher, _marker: PhantomData }
	}

	/// matches a big endian `bytes` word by a bits [`Matcher`].
	///
	/// `word_be` takes a [`LBits`]/[`BBits`] `matcher` and produces a [`Matcher`] that matches a big endian byte section of `bytes` length, then matches its bits in little / big endian order with `matcher`, propagating its result.
	///
	/// # example
	/// ```
	/// let bit_matcher = matcher!(for BBits, (a = _[4]) {b(8, 0xaa)} (b = _[4]));
	/// assert_eq!(
	///     try_match(&[0x1a, 0xa2][..], word_be(2, bit_matcher)),
	///     Some((BBits::new(4, 1), BBits::new(4, 2)))
	/// );
	/// assert!(!matches(&[0x1b, 0xa2][..], word_be(2, bit_matcher)));
	/// assert!(!matches(&[0xa1][..], word_be(2, bit_matcher)));
	/// ```
	pub fn word_be<T: BitsType, M: Matcher<T>>(bytes: u8, matcher: M) -> WordBE<T, M> {
		assert!(bytes <= 8 && bytes != 0);
		WordBE { bytes, matcher, _marker: PhantomData }
	}

	/// [`word_le`] [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct WordLE<T, U> {
		bytes: u8,
		matcher: U,
		_marker: PhantomData<T>,
	}

	/// [`word_be`] [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct WordBE<T, U> {
		bytes: u8,
		matcher: U,
		_marker: PhantomData<T>,
	}

	macro_rules! word_xe {
		($word_xe:ident, $XBits:ident, $from_bytes:ident, $WordXE:ident) => {
			impl<U, C> Matcher<[u8]> for $WordXE<$XBits, U>
			where
				for<'src> U: Matcher<$XBits, Capture<'src> = C>,
			{
				type Capture<'src> = C;
				fn do_match<'src, M: Mode>(
					&self, matched: &'src [u8], off: &mut usize,
				) -> MatchResult<Self::Capture<'static>, M> {
					let Some(slice) = matched.get(*off..*off + self.bytes as usize)
					else {
						*off = matched.len();
						return M::err(|| MatchError::mismatch(self.expected(), *off));
					};
					let bits = $XBits(Bits::$from_bytes(slice));
					match self.matcher.do_match::<M>(&bits, &mut 0) {
						Ok(suc) => {
							*off += self.bytes as usize;
							Ok(suc)
						}
						Err(bit_err) => M::err(|| {
							let mut err = LeanString::new();
							let bit_err = M::unwrap_error(bit_err);
							write!(err, "while matching word at {off}: {bit_err}")
								.unwrap();
							MatchError::other(err, *off)
						}),
					}
				}
			}
		};
	}
	word_xe!(word_le, LBits, from_le_slice, WordLE);
	word_xe!(word_le, BBits, from_le_slice, WordLE);
	word_xe!(word_be, LBits, from_be_slice, WordBE);
	word_xe!(word_be, BBits, from_be_slice, WordBE);
}
#[cfg(feature = "bits")]
pub use bits_ext::{word_be, word_le};
