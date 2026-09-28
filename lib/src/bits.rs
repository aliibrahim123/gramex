//! bits matching implementation
//!
//! when `bits` feature is enabled, bits matching gets enabled with additional extra [`Matcher`]s through this module.
//!
//! # [`Bits`]
//! [`Bits`] is the core structure in bit matching, it represents a bitfield of maximum 64 bits.
//!
//! [`Bits`] can be converted from and to all integer types, byte arrays and bools.
//!
//! it is created through the [`b`] function.
//!
//! ```
//! assert_eq!(b(3, 0b101), Bits { value: 0b101, len: 3 });
//! assert_eq!(Bits::from(0x1234u16), b(16, 0x1234));
//! assert_eq!(u32::try_from(b(32, 0x1234)), Ok(0x1234));
//! assert_eq!(b(5, 0b10101), 0b10101u64);
//! ```
//!
//! # [`MatchAble`] implementation
//! bit matching is done through 2 [`Bits`] wrappers: [`BBits`] and [`LBits`], for big endian and little endian directions respectively.
//!
//! [`MatchAble`] is implemented for each where [`Slice`](MatchAble::Slice) is [`LBits`]/[`BBits`], [`Token`](MatchAble) is [`bool`], and offsets are bit position.
//!
//! ```
//! let bits = LBits::new(5, 0b10101);
//! assert_eq!(MatchAble::len(&bits), 5);
//!
//! assert_eq!(MatchAble::slice(&bits, 1..3), Some(LBits::new(2, 0b10)));
//! assert_eq!(MatchAble::slice(&bits, 4..6), None);
//! assert_eq!(MatchAble::get_token(&bits, 2), Some(true));
//! assert_eq!(MatchAble::get_token(&bits, 5), None);
//!
//! let mut off = 2;
//! assert!(MatchAble::skip_n::<Test>(&bits, &mut off, 2).is_ok());
//! assert_eq!(off, 4);
//! ```
//!
//! # core [`Matcher`]s
//! for each endian wrapper, [`Matcher`] is implemented for [`Bits`], [`bool`], `{i,u}{8,16,32}`. in addition, for the universal `&Bits`, [`Box<Bits>`], [`Rc<Bits>`](alloc_crate::rc::Rc) and [`Arc<Bits>`](alloc_crate::sync::Arc).
//!
//! range matching is implemented through [`BitRange`], created by the [`br`] function.
//!
//! all these [`Matcher`]s match against the endian wrapper slice.
//!
//! ```
//! let bits = LBits::new(5, 0b10101);
//! assert_eq!(parse(&bits, b(5, 0b10101)), Ok(LBits::new(5, 0b10101)));
//! assert_eq!(
//!     parse(&bits, b(5, 0b10111)),
//!     Err(MatchError::mismatch("`10111`".into(), 0))
//! );
//! assert_eq!(
//!     parse(&LBits::new(3, 0b101), b(5, 0b10111)),
//!     Err(MatchError::incomplete("`10111`".into(), 0))
//! );
//!
//! assert!(true.test(&bits, &mut 0));
//! assert!(matches(&LBits::new(8, 0b00001111), 15u8));
//! assert_eq!(matches(&bits, Box::new(b(5, 0b10101))));
//!
//! assert_eq!(matches(bits, br(5, 0b01000..=0b11111)));
//! ```
use core::{
	fmt::{self, Binary, Formatter, Write},
	ops::{Range, RangeInclusive},
};

use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	derive::{define_slice_matcher, define_token_matcher, match_slice},
	result::{Expected, MatchResult},
};

/// a bitfield.
///
/// `Bits` is a bitfield of a maximum of 64 bits, able to be matched with gramex.
///
/// it is created through the [`b`] function, and it is a core [`Matcher`] for [`LBits`] and [`BBits`].
///
/// it is convertible from and to:
/// - [`u8`], [`u16`], [`u32`], [`u64`], [`usize`].
/// - [`i8`], [`i16`], [`i32`], [`i64`], [`isize`].
/// - [`bool`], [`f32`], [`f64`].
///
/// it is comparable to [`u64`].
///
/// # example
/// ```
/// assert_eq!(b(5, 0b10101), Bits { value: 0b10101, len: 5 });
/// assert_eq!(Bits::default(), Bits { value: 0, len: 0 });
///
/// assert_eq!(Bits::from(0x1234u16), b(16, 0x1234));
/// assert_eq!(Bits::from(true), b(1, 1));
///
/// assert_eq!(u32::try_from(b(32, 0x1234)), Ok(0x1234));
/// assert_eq!(u32::try_from(b(16, 0x1234)), Ok(0x1234));
/// assert_eq!(u32::try_from(b(48, 0x1234)), Err(()));
///
/// assert_eq!(b(5, 0b10101), 0b10101u64);
/// assert_eq!(format!("{:b}", b(5, 0b10101)), "0b10101");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bits {
	/// the value of the bitfield
	pub value: u64,
	/// the length of the bitfield
	pub len: u8,
}

/// create a [`Bits`] of given `value` and `len`.
///
/// it panics if `len` is outside `1..=64`, or `value` is greater than `len` capacity.
///
/// # example
/// ```
/// assert_eq!(b(5, 0b10101), Bits { value: 0b10101, len: 5 });
/// assert_eq!(
///     try_match(&LBits::new(5, 0b10101),
///     b(5, 0b10101)), Ok(LBits::new(5, 0b10101))
/// );
/// ```
#[inline]
pub fn b(len: u8, value: u64) -> Bits {
	assert!(len <= 64 && len != 0);
	assert!(value <= u64::MAX >> (64 - len));
	Bits { value, len }
}
impl Bits {
	#[inline]
	fn len(&self) -> usize {
		self.len as usize
	}
}
impl Default for Bits {
	fn default() -> Self {
		Self { value: 0, len: 0 }
	}
}

macro_rules! int_conv {
	[$($ty:ty $(as $as:ty)?),+] => {
		$(
			impl From<$ty> for Bits {
				#[inline]
				fn from(value: $ty) -> Self {
					Self {
						value: value $(as $as)? as u64,
						len: (size_of::<$ty>() as u8) * 8
					}
				}
			}
			impl TryFrom<Bits> for $ty {
				type Error = ();
				#[inline]
				fn try_from(bits: Bits) -> Result<Self, Self::Error> {
					if bits.len as usize > size_of::<$ty>() * 8 {
						return Err(());
					}
					Ok(bits.value as $ty)
				}
			}
		)+
	};
}
#[rustfmt::skip]
int_conv![
	u8, u16, u32, i8 as u8, i16 as u16, i32 as u32, usize, isize as usize
];

impl From<bool> for Bits {
	#[inline]
	fn from(value: bool) -> Self {
		Self { value: value as u64, len: 1 }
	}
}
impl TryFrom<Bits> for bool {
	type Error = ();
	#[inline]
	fn try_from(bits: Bits) -> Result<Self, Self::Error> {
		if bits.len != 1 {
			return Err(());
		}
		Ok(bits.value != 0)
	}
}

macro_rules! float_conv {
	[$($ty:ty),+] => {
		$(
			impl From<$ty> for Bits {
				#[inline]
				fn from(value: $ty) -> Self {
					Self {
						value: value.to_bits() as u64,
						len: (size_of::<$ty>() as u8) * 8
					}
				}
			}
			impl TryFrom<Bits> for $ty {
				type Error = ();
				#[inline]
				fn try_from(bits: Bits) -> Result<Self, Self::Error> {
					if bits.len as usize > size_of::<$ty>() * 8 {
						return Err(());
					}
					Ok(<$ty>::from_bits(bits.value as _))
				}
			}
		)+
	};
}
float_conv![f32, f64];

impl Bits {
	pub(crate) fn from_le_slice(value: &[u8]) -> Self {
		let mut buf = [0u8; 8];
		buf[..value.len()].copy_from_slice(&value);
		Self { value: u64::from_le_bytes(buf), len: (value.len() * 8) as u8 }
	}
	pub(crate) fn from_be_slice(value: &[u8]) -> Self {
		let mut buf = [0u8; 8];
		buf[..value.len()].copy_from_slice(&value);
		buf[..value.len()].reverse();
		Self { value: u64::from_le_bytes(buf), len: (value.len() * 8) as u8 }
	}
	/// convert a little endian byte array into `Bits`.
	///
	/// `N` must be `1..=8`.
	///
	/// # example
	/// ```
	/// assert_eq!(Bits::from_le_bytes([1, 2]), b(16, 0x201));
	/// ```
	pub fn from_le_bytes<const N: usize>(value: [u8; N]) -> Self {
		assert!(N <= 8);
		Self::from_le_slice(&value)
	}

	/// convert a big endian byte array into `Bits`.
	///
	/// `N` must be `1..=8`.
	///
	/// # example
	/// ```
	/// assert_eq!(Bits::from_be_bytes([1, 2]), b(16, 0x102));
	/// ```
	pub fn from_be_bytes<const N: usize>(value: [u8; N]) -> Self {
		assert!(N <= 8);
		Self::from_be_slice(&value)
	}

	/// convert `Bits` into little endian byte array.
	///
	/// `N` must be `1..=8` and it return `None` if the `len` is larger than the array capacity
	///
	/// # example
	/// ```
	/// assert_eq!(b(16, 0x201).to_le_bytes::<2>(), Some([1, 2]));
	/// assert_eq!(b(32, 0x201).to_le_bytes::<2>(), None);
	/// ```
	pub fn to_le_bytes<const N: usize>(self) -> Option<[u8; N]> {
		assert!(N <= 8);
		if self.len > (N * 8) as u8 {
			return None;
		}
		Some(self.value.to_le_bytes()[..N].try_into().unwrap())
	}

	/// convert `Bits` into big endian byte array.
	///
	/// `N` must be `1..=8` and it return `None` if the `len` is larger than the array capacity
	///
	/// # example
	/// ```
	/// assert_eq!(b(16, 0x102).to_be_bytes::<2>(), Some([1, 2]));
	/// assert_eq!(b(32, 0x102).to_be_bytes::<2>(), None);
	/// ```
	pub fn to_be_bytes<const N: usize>(self) -> Option<[u8; N]> {
		assert!(N <= 8);
		if self.len > (N * 8) as u8 {
			return None;
		}
		Some(self.value.to_be_bytes()[8 - N..].try_into().unwrap())
	}
}

impl Binary for Bits {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "0b{:01$b}", self.value, self.len as usize)
	}
}

/// little endian [`Bits`] wrapper.
///
/// `LBits` is a wrapper around [`Bits`] which implements [`MatchAble`] in the little-endian direction.
///
/// see the [module documentation](crate::bits) for more info.
///
/// it is convertible and comparable between [`Bits`] and [`BBits`].
///
/// # example
/// ```
/// let bits = LBits::from(b(5, 0b10101));
/// assert_eq!(bits, LBits(Bits { value: 0b10101, len:5 }));
/// assert_eq!(bits, b(5, 0b10101));
/// assert!(matches(&bits, b5(5, 0b10101)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LBits(pub Bits);

/// big endian [`Bits`] wrapper.
///
/// `BBits` is a wrapper around [`Bits`] which implements [`MatchAble`] in the big-endian direction.
///
/// see the [module documentation](crate::bits) for more info.
///
/// it is convertible and comparable between [`Bits`] and [`LBits`].
///
/// # example
/// ```
/// let bits = BBits::from(b(5, 0b10101));
/// assert_eq!(bits, BBits(Bits { value: 0b10101, len:5 }));
/// assert_eq!(bits, b(5, 0b10101));
/// assert!(matches(&bits, b5(5, 0b10101)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BBits(pub Bits);

impl LBits {
	/// create a new `LBits` of given `value` and `len`.
	///
	/// panics if `len` is outside `1..=64`.
	#[inline]
	pub fn new(len: u8, value: u64) -> LBits {
		LBits(b(len, value))
	}
}
impl BBits {
	#[inline]
	/// create a new `BBits` of given `value` and `len`.
	///
	/// panics if `len` is outside `1..=64`.
	pub fn new(len: u8, value: u64) -> BBits {
		BBits(b(len, value))
	}
}

macro_rules! impl_from {
	[$($from:ident -> $to:ident: $value:ident => $logic:expr),+] => {
		$(impl From<$from> for $to {
			#[inline]
			fn from($value: $from) -> Self {
				$logic
			}
		})+
	}
}
impl_from![
	Bits -> BBits: value => BBits(value),
	Bits -> LBits: value => LBits(value),
	BBits -> Bits: value => value.0,
	LBits -> Bits: value => value.0,
	LBits -> BBits: value => BBits(value.0),
	BBits -> LBits: value => LBits(value.0),

	u64 -> Bits: value => Bits { value, len: 64 },
	Bits -> u64: value => value.value,
	i64 -> Bits: value => Bits { value: value as u64, len: 64 },
	Bits -> i64: value => value.value as i64,
	u64 -> LBits: value => LBits(Bits::from(value)),
	u64 -> BBits: value => BBits(Bits::from(value)),
	LBits -> u64: value => value.0.value,
	BBits -> u64: value => value.0.value,
	i64 -> LBits: value => LBits(Bits::from(value)),
	i64 -> BBits: value => BBits(Bits::from(value)),
	LBits -> i64: value => value.0.value as i64,
	BBits -> i64: value => value.0.value as i64

];

macro_rules! steal_bits_from {
	[$($(#for ($($args:tt)+))? $ty:ty),+] => {
		$(
			impl $(<$($args)+>)? From<$ty> for LBits {
				#[inline]
				fn from(value: $ty) -> Self {
					Self(Bits::from(value))
				}
			}
			impl $(<$($args)+>)? From<$ty> for BBits {
				#[inline]
				fn from(value: $ty) -> Self {
					Self(Bits::from(value))
				}
			}
			impl $(<$($args)+>)? TryFrom<LBits> for $ty {
				type Error = ();
				#[inline]
				fn try_from(value: LBits) -> Result<Self, Self::Error> {
					<$ty>::try_from(value.0)
				}
			}
			impl $(<$($args)+>)? TryFrom<BBits> for $ty {
				type Error = ();
				#[inline]
				fn try_from(value: BBits) -> Result<Self, Self::Error> {
					<$ty>::try_from(value.0)
				}
			}
		)+
	};
}
steal_bits_from![u8, u16, u32, i8, i16, i32, usize, isize, bool, f32, f64];

macro_rules! steal_bytes_conv {
	($ty:ident) => {
		impl $ty {
			pub fn to_le_bytes<const N: usize>(self) -> Option<[u8; N]> {
				self.0.to_le_bytes()
			}
			pub fn to_be_bytes<const N: usize>(self) -> Option<[u8; N]> {
				self.0.to_be_bytes()
			}
			pub fn from_be_bytes<const N: usize>(value: [u8; N]) -> $ty {
				Bits::from_be_bytes(value).into()
			}
			pub fn from_le_bytes<const N: usize>(value: [u8; N]) -> $ty {
				Bits::from_le_bytes(value).into()
			}
		}
	};
}
steal_bytes_conv!(LBits);
steal_bytes_conv!(BBits);

macro_rules! impl_partial_eq {
	[$(($lhs:ident :$lhs_t:ty, $rhs:ident :$rhs_t:ty) => $logic:expr),+] => {
		$(impl PartialEq<$rhs_t> for $lhs_t {
			#[inline]
			fn eq(&self, $rhs: &$rhs_t) -> bool {
				let $lhs = self;
				$logic
			}
		})+
	};
}
impl_partial_eq![
	(a: Bits, b: u64) => a.value == *b,
	(a: u64, b: Bits) => *a == b.value,
	(a: LBits, b: Bits) => a.0 == *b,
	(a: Bits, b: LBits) => *a == b.0,
	(a: BBits, b: Bits) => a.0 == *b,
	(a: Bits, b: BBits) => *a == b.0,
	(a: LBits, b: BBits) => a.0 == b.0,
	(a: BBits, b: LBits) => a.0 == b.0,
	(a: LBits, b: u64) => a.0.value == *b,
	(a: u64, b: LBits) => *a == b.0.value,
	(a: BBits, b: u64) => a.0.value == *b,
	(a: u64, b: BBits) => *a == b.0.value
];

impl Binary for LBits {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "0b{:01$b}", self.0.value, self.0.len as usize)
	}
}
impl Binary for BBits {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "0b{:01$b}", self.0.value, self.0.len as usize)
	}
}

/// bit extract little endian
#[inline]
fn bit_extract_le(value: u64, start: u8, end: u8) -> u64 {
	if start == end {
		return 0;
	}
	(value >> start) & (u64::MAX >> (64 - (end - start)))
}
#[inline]
/// bit extract big endian
fn bit_extract_be(value: u64, start: u8, end: u8, len: u8) -> u64 {
	bit_extract_le(value, len - end, len - start)
}

/// value + len -> "`0100101`"
fn to_bin(value: u64, len: u8) -> LeanString {
	let mut str = LeanString::new();
	write!(str, "`{value:00$b}`", len as usize).unwrap();
	str
}

/// range of [`Bits`] field.
///
/// it is a [`RangeInclusive`] equivalent for [`Bits`] facilitating bit range matching.
///
/// it is created by the [`br`] function.
///
/// # example
/// ```
/// let bits = LBits::new(5, 0b10101);
/// assert!(matches(&bits, br(5, 0b01000..=0b11111)));
/// assert!(!matches(&bits, br(5, 0b00000..=0b01000)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitRange {
	/// the start value of the `BitRange`
	pub start: u64,
	/// the end value of the `BitRange`, inclusive
	pub end: u64,
	/// the length of the `BitRange`
	pub len: u8,
}
impl BitRange {
	#[inline]
	fn len(&self) -> usize {
		self.len as usize
	}
}

/// create a new [`BitRange`] given a `len` and a `range`.
///
/// it panics if `len` is outside `1..=64`, or `start` or `end` are larger than `len` capacity.
///
/// # example
/// ```
/// let bits = LBits::new(5, 0b10101);
/// assert!(matches(&bits, br(5, 0b01000..=0b11111)));
/// assert!(!matches(&bits, br(5, 0b00000..=0b01000)));
/// ```
#[inline]
pub fn br(len: u8, range: RangeInclusive<u64>) -> BitRange {
	assert!(len <= 64);
	assert!(*range.start() <= u64::MAX >> (64 - len));
	assert!(*range.end() <= u64::MAX >> (64 - len));
	BitRange { start: *range.start(), end: *range.end(), len }
}

macro_rules! into_bits_matchers {
	($matchable:ident, [$($(#for ($($t:tt)+))? $ty:ty),+]) => {
		$(impl $(<$($t)+>)? Matcher<$matchable> for $ty {
			type Capture<'src> = $matchable;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src $matchable, off: &mut usize,
			) -> MatchResult<$matchable, M> {
				Bits::from(*self).do_match::<M>(matched, off)
			}
			fn expected(&self) -> Expected {
				Expected::A(to_bin(Bits::from(*self).value, size_of::<$ty>() as u8 * 8))
			}
		})+
	};
}

macro_rules! impl_matching {
	($ty:ident, $bit_extract:ident $((.., $len:tt))?) => {
		impl MatchAble for $ty {
			type Slice<'src> = $ty;
			type Token<'src> = bool;

			#[inline]
			fn len(&self) -> usize {
				self.0.len as usize
			}
			#[inline]
			fn get_token<'src>(&'src self, off: usize) -> Option<bool> {
				if off >= self.0.len as usize {
					return None;
				}
				Some($bit_extract(
					self.0.value, off as u8, (off as u8) + 1, $(self.0.$len)?
				) != 0)
			}
			#[inline]
			fn slice<'src>(&'src self, range: Range<usize>) -> Option<$ty> {
				if range.end > self.0.len as usize {
					return None;
				}
				Some($ty(Bits {
					value: $bit_extract(
						self.0.value, range.start as u8, range.end as u8, $(self.0.$len)?
					),
					len: (range.end - range.start) as u8,
				}))
			}
		}
		define_slice_matcher!(Bits, $ty, |matcher, field| (
			field.0.value == matcher.value,
			Expected::A(to_bin(matcher.value, matcher.len))
		));
		define_token_matcher!(bool, $ty, |matcher, bit| (
			(bit == *matcher).then_some(1),
			Expected::A(to_bin(*matcher as u64, 1))
		));
		into_bits_matchers!($ty, [
			u8, u16, u32, i8, i16, i32
		]);
		define_slice_matcher!(BitRange, $ty, |matcher, field| (
			(matcher.start..=matcher.end).contains(&field.0.value),
			Expected::Between(
				to_bin(matcher.start, matcher.len),
				to_bin(matcher.end, matcher.len)
			)
		));
		impl<F: Fn(u64) -> bool> Matcher<$ty> for A<F> {
			type Capture<'src> = $ty;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src $ty, off: &mut usize,
			) -> MatchResult<$ty, M> {
				match_slice::<M, _, _>(matched, off, self.len as usize,
					|slice| (self.fun)(slice.0.value).then_some(slice),
					|| <_ as Matcher<$ty>>::expected(&self)
				)
			}
			fn expected(&self) -> Expected {
				let mut str = LeanString::new();
				write!(str, "a {}-bit value", self.len).unwrap();
				Expected::A(str)
			}
		}
	};
}
impl_matching!(LBits, bit_extract_le);
impl_matching!(BBits, bit_extract_be(.., len));

/// matches a `len` sized [`Bits`] field by a predicate.
///
/// `a_b` produces a [`Matcher`] that matches a `len` sized [`Bits`] field, then tests its value by `pred`; if `pred` returns `true` it matches with the field, else it fails.
///
/// # example
/// ```
/// assert_eq!(
///     try_match(&LBits::new(8, 0x12), a_b(8, |x| x > 0x10)),
///     Ok(LBits::new(8, 0x12))
/// );
/// assert_eq!(!matches(&LBits::new(8, 0x1), a_b(8, |x| x > 0x10)));
/// ```
pub fn a_b<F: Fn(u64) -> bool>(len: u8, fun: F) -> A<F> {
	assert!(len <= 64 && len != 0);
	A { fun, len }
}

/// [`a_b`] [`Matcher`]
#[derive(Debug, Clone, Copy)]
pub struct A<F> {
	fun: F,
	len: u8,
}
