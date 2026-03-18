//! # bytes matching
//! `gramex` supports byte matching through `&[u8]` [byte](u8) [slices](primitive@slice).
//!
//! `&[u8]` implements [`MatchBy`] for [`u8`], [`&u8`](u8), [`&[u8]`](primitive@slice), [`[u8; N]`](primitive@array), [`&[u8; N]`](primitive@array) and [`&Vec<u8>`](Vec<u8>).
//!
//! it also support range matching by [`u8`].
//!
//! ```
//! assert!(matches!([1, 2, 3]: [u8], 1 0x02 0b0000_0011));
//! assert!(matches!([1, 2, 3]: [u8], {[1u8, 2, 3]}));
//! assert!(matches!([1, 2, 3]: [u8], 1? !0xff 0x00..0x7f));
//! ```
//!
//! it also support matching by any type implementing [`AsRef<[u8]>`](AsRef) through [`bytes_of`].
//! ```
//! assert!(matches!(b"abc": [u8], {bytes_of("abc")}));
//! ```
//!
//! integers can be bytewise matched through [`{int.to_(le|be)_bytes()}`](u64::to_le_bytes).
//! ```
//! assert!(matches!([1, 0, 0, 0]: [u8], {1u32.to_le_bytes()}));
//! ```
//!
//! # bits matching
//! `gramex` supports bit matching through [`Bits`].
//!
//! [`Bits`] is a bitfield wrapper supporting bit matching upto 64 bits, from most to least significant bit.
//!
//! it supports matching by [`Bits`] and [`&Bits`](Bits), through the `bn` functions that creates an `n` sized [`Bits`] field.
//!
//! it also support range matching through the `bnr` functions.
//!
//! ```
//! assert!(matches!(b8(0x12): Bits, {b4(0x1)} {b2(0)} {b2(0b11)}));
//! assert!(matches!(b12(0x123): Bits, {b4(0x1)}? {b4(0)}+ {b4r(0..=7)}));
//! ```
//!
//! `gramex` also support bit matching in the other direction (from least to most significant bit) through the [`LBits`] wrapper.
//!
//! ```
//! assert!(matches!(b8(0x12): LBits, {b4(0x2)} {b2(0b01)} {b2(0)}));
//! ```

use std::{
	fmt::Display,
	ops::{Range, RangeInclusive},
};

use crate::{MatchAble, MatchBy, MatchSignal, MatchStatus, Matcher};

impl MatchAble for [u8] {
	type Slice<'a> = &'a [u8];
	#[inline]
	fn len(&self) -> usize {
		self.len()
	}
	#[inline]
	fn slice(&self, range: Range<usize>) -> Self::Slice<'_> {
		&self[range]
	}
}

impl MatchBy<u8> for [u8] {
	#[inline]
	fn match_by(&self, matcher: u8, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + 1 > self.len() {
			MatchSignal::InComplete
		} else if self[*ind] == matcher {
			*ind += 1;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a> MatchBy<&'a u8> for [u8] {
	#[inline]
	fn match_by(&self, matcher: &'a u8, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + 1 > self.len() {
			MatchSignal::InComplete
		} else if self[*ind] == *matcher {
			*ind += 1;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}

impl<'a> MatchBy<&'a [u8]> for [u8] {
	#[inline]
	fn match_by(&self, matcher: &'a [u8], ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len() > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(matcher) {
			*ind += matcher.len();
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a, const N: usize> MatchBy<&'a [u8; N]> for [u8] {
	#[inline]
	fn match_by(
		&self, matcher: &'a [u8; N], ind: &mut usize, _status: &MatchStatus,
	) -> MatchSignal {
		if *ind + matcher.len() > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(matcher) {
			*ind += matcher.len();
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<const N: usize> MatchBy<[u8; N]> for [u8] {
	#[inline]
	fn match_by(&self, matcher: [u8; N], ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len() > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(&matcher) {
			*ind += matcher.len();
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a> MatchBy<&'a Vec<u8>> for [u8] {
	#[inline]
	fn match_by(
		&self, matcher: &'a Vec<u8>, ind: &mut usize, _status: &MatchStatus,
	) -> MatchSignal {
		if *ind + matcher.len() > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(matcher) {
			*ind += matcher.len();
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}

impl MatchBy<RangeInclusive<u8>> for [u8] {
	#[inline]
	fn match_by(
		&self, matcher: RangeInclusive<u8>, ind: &mut usize, _status: &MatchStatus,
	) -> MatchSignal {
		if *ind + 1 > self.len() {
			MatchSignal::InComplete
		} else if matcher.contains(&self[*ind]) {
			*ind += 1;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}

/// return the bytes of the value.
///
/// it is a shortcut for [`AsRef<[u8]>::as_ref`](AsRef::as_ref).
///
/// # example
/// ```
/// assert!(matches!(b"abc": [u8], {bytes_of("abc")}));
/// ```
#[inline]
pub fn bytes_of<T: AsRef<[u8]> + ?Sized>(value: &T) -> &[u8] {
	value.as_ref()
}

/// a sized bit field.
///
/// `Bits` is a [`u64`] with length, it is the [`MatchAble`] that is used in bit matching.
///
/// it can be sized from 1 to 64 bits, and indexed by bit from most significant bit to the lowest one.
///
/// can be created / converted from and into any int, little endian [`[u8; N]`](primitive@array) and [`&[u8]`](primitive@slice).
///
/// # example
/// ```
/// let bits = Bits::from(0x123u32);
/// assert_eq!(bits, Bits { value: 0x123, len: 32 });
/// assert_eq!(bits.try_into(), Ok(0x123i32));
///
/// let bits = Bits::from([0x12, 0x34]);
/// assert_eq!(bits, Bits { value: 0x3412, len: 16 });
/// assert_eq!(bits.try_into(), Ok([0x12, 0x34]));
///
/// assert!(matches!(bits: Bits, {b8(0x34)} {b8(0x12)}));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bits {
	pub value: u64,
	pub len: usize,
}
/// a sized bit field, in the other direction.
///
/// `LBits` is a [`Bits`] but indexed from least significant bit to the most significant one.
///
/// it is used in bit matching in the reverse order.
///
/// convertable between the types [`Bits`] is convertable to, in addition to and from [`Bits`] (with no change in value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LBits {
	pub value: u64,
	pub len: usize,
}
impl From<Bits> for LBits {
	fn from(value: Bits) -> Self {
		LBits { value: value.value, len: value.len }
	}
}
impl From<LBits> for Bits {
	fn from(value: LBits) -> Self {
		Bits { value: value.value, len: value.len }
	}
}
impl PartialEq<u64> for Bits {
	fn eq(&self, other: &u64) -> bool {
		self.value == *other
	}
}
impl PartialEq<u64> for LBits {
	fn eq(&self, other: &u64) -> bool {
		self.value == *other
	}
}
impl PartialEq<LBits> for Bits {
	fn eq(&self, other: &LBits) -> bool {
		self.value == other.value && self.len == other.len
	}
}

/// error occuring when [`Bits`] is converted from / to a smaller type.
///
/// this error occurs when the value of [`Bits`] can not fit into the target type, based on the [`Bits.length`] field, even if the value can fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitOverflowError {
	pub target_len: usize,
	pub value_len: usize,
}
impl Display for BitOverflowError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let Self { target_len, value_len } = self;
		write!(f, "target length {target_len} is smaller than value length {value_len}",)
	}
}
macro_rules! bits_conv {
	[$($ty:ty), +] => {
		$(
			impl From<$ty> for Bits {
				fn from(value: $ty) -> Self {
					Bits { value: value as u64, len: size_of::<$ty>() * 8 }
				}
			}
			impl TryInto<$ty> for Bits {
				type Error = BitOverflowError;
				fn try_into(self) -> Result<$ty, BitOverflowError> {
					if self.len > size_of::<$ty>() * 8 {
						return Err(BitOverflowError {
							target_len: size_of::<$ty>() * 8,
							value_len: self.len
						});
					}
					Ok(self.value as $ty)
				}
			}
			impl From<$ty> for LBits {
				fn from(value: $ty) -> Self {
					LBits { value: value as u64, len: size_of::<$ty>() * 8 }
				}
			}
			impl TryInto<$ty> for LBits {
				type Error = BitOverflowError;
				fn try_into(self) -> Result<$ty, BitOverflowError> {
					if self.len > size_of::<$ty>() * 8 {
						return Err(BitOverflowError {
							target_len: size_of::<$ty>() * 8,
							value_len: self.len
						});
					}
					Ok(self.value as $ty)
				}
			}
		)+
	};
}
bits_conv![u8, u16, u32, u64, i8, i16, i32, i64];
impl<const N: usize> From<[u8; N]> for Bits {
	fn from(value: [u8; N]) -> Self {
		assert!(N <= 8);
		let mut buf = [0u8; 8];
		buf[..N].copy_from_slice(&value);
		Bits { value: u64::from_le_bytes(buf), len: N * 8 }
	}
}
impl<const N: usize> TryInto<[u8; N]> for Bits {
	type Error = BitOverflowError;
	fn try_into(self) -> Result<[u8; N], BitOverflowError> {
		if self.len > N * 8 {
			return Err(BitOverflowError { target_len: N * 8, value_len: self.len });
		}
		let mut buf = [0u8; N];
		buf.copy_from_slice(&self.value.to_le_bytes()[..N]);
		Ok(buf)
	}
}
impl<const N: usize> From<[u8; N]> for LBits {
	fn from(value: [u8; N]) -> Self {
		assert!(N <= 8);
		let mut buf = [0u8; 8];
		buf[..N].copy_from_slice(&value);
		LBits { value: u64::from_le_bytes(buf), len: N * 8 }
	}
}
impl<const N: usize> TryInto<[u8; N]> for LBits {
	type Error = BitOverflowError;
	fn try_into(self) -> Result<[u8; N], BitOverflowError> {
		if self.len > N * 8 {
			return Err(BitOverflowError { target_len: N * 8, value_len: self.len });
		}
		let mut buf = [0u8; N];
		buf.copy_from_slice(&self.value.to_le_bytes()[..N]);
		Ok(buf)
	}
}
impl TryFrom<&[u8]> for Bits {
	type Error = BitOverflowError;
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		if value.len() > 8 {
			return Err(BitOverflowError { target_len: 64, value_len: value.len() * 8 });
		}
		let mut buf = [0u8; 8];
		buf[..value.len()].copy_from_slice(value);
		Ok(Bits { value: u64::from_le_bytes(buf), len: value.len() * 8 })
	}
}
impl Bits {
	pub fn new(value: u64, len: usize) -> Bits {
		Bits { value, len }
	}
}
impl TryFrom<&[u8]> for LBits {
	type Error = BitOverflowError;
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		if value.len() > 8 {
			return Err(BitOverflowError { target_len: 64, value_len: value.len() * 8 });
		}
		let mut buf = [0u8; 8];
		buf[..value.len()].copy_from_slice(value);
		Ok(LBits { value: u64::from_le_bytes(buf), len: value.len() * 8 })
	}
}
impl LBits {
	pub fn new(value: u64, len: usize) -> LBits {
		LBits { value, len }
	}
}
impl std::fmt::Binary for Bits {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = format!("{:064b}", self.value);
		write!(f, "0b{}", &s[64 - self.len..])
	}
}
impl std::fmt::Binary for LBits {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = format!("{:064b}", self.value);
		write!(f, "0b{}", &s[64 - self.len..])
	}
}
impl AsRef<Bits> for Bits {
	fn as_ref(&self) -> &Bits {
		self
	}
}
impl AsRef<LBits> for LBits {
	fn as_ref(&self) -> &LBits {
		self
	}
}
/// extract bits from a [`u64`], indexed from least significant bit
fn bitextract_lsb(value: u64, start: usize, end: usize) -> u64 {
	(value >> start) & u64::MAX >> (64 - (end - start))
}
/// extract bits from a [`u64`], indexed from most significant bit.
///
/// of `len` size occuping the low.
fn bitextract_msb(value: u64, len: usize, start: usize, end: usize) -> u64 {
	(value >> (len - end)) & (u64::MAX >> (64 - (end - start)))
}
impl MatchAble for Bits {
	type Slice<'a> = Bits;
	#[inline]
	fn len(&self) -> usize {
		self.len
	}
	#[inline]
	fn slice(&self, range: Range<usize>) -> Bits {
		let Range { start, end } = range;
		if start == end {
			return Bits { value: 0, len: 0 };
		}
		Bits { value: bitextract_msb(self.value, self.len, start, end), len: end - start }
	}
	#[inline]
	fn get_n(
		&self, ind: &mut usize, n: usize, _status: &MatchStatus,
	) -> Result<Self::Slice<'_>, MatchSignal> {
		if *ind + n > self.len {
			Err(MatchSignal::InComplete)
		} else {
			*ind += n;
			Ok(self.slice(*ind - n..*ind))
		}
	}
	#[inline]
	fn skip_n(&self, ind: &mut usize, n: usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + n > self.len {
			MatchSignal::InComplete
		} else {
			*ind += n;
			MatchSignal::Matched
		}
	}
}
impl MatchBy<Bits> for Bits {
	#[inline]
	fn match_by(&self, matcher: Bits, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len > self.len {
			MatchSignal::InComplete
		} else if bitextract_msb(self.value, self.len, *ind, *ind + matcher.len) == matcher.value {
			*ind += matcher.len;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}

impl<'a> MatchBy<&'a Bits> for Bits {
	#[inline]
	fn match_by(&self, matcher: &'a Bits, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		Bits::match_by(self, *matcher, ind, status)
	}
}

impl MatchAble for LBits {
	type Slice<'a> = LBits;
	#[inline]
	fn len(&self) -> usize {
		self.len
	}
	#[inline]
	fn slice(&self, range: Range<usize>) -> LBits {
		let Range { start, end } = range;
		if start == end {
			return LBits { value: 0, len: 0 };
		}
		LBits { value: bitextract_lsb(self.value, start, end), len: end - start }
	}
	#[inline]
	fn get_n(
		&self, ind: &mut usize, n: usize, _status: &MatchStatus,
	) -> Result<Self::Slice<'_>, MatchSignal> {
		if *ind + n > self.len {
			Err(MatchSignal::InComplete)
		} else {
			*ind += n;
			Ok(self.slice(*ind - n..*ind))
		}
	}
	#[inline]
	fn skip_n(&self, ind: &mut usize, n: usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + n > self.len {
			MatchSignal::InComplete
		} else {
			*ind += n;
			MatchSignal::Matched
		}
	}
}
impl MatchBy<Bits> for LBits {
	#[inline]
	fn match_by(&self, matcher: Bits, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len > self.len {
			MatchSignal::InComplete
		} else if bitextract_lsb(self.value, *ind, *ind + matcher.len) == matcher.value {
			*ind += matcher.len;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a> MatchBy<&'a Bits> for LBits {
	#[inline]
	fn match_by(&self, matcher: &'a Bits, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		LBits::match_by(self, *matcher, ind, status)
	}
}
/// a `RangeInclusive` of bits.
///
/// used to implement range matching for [`Bits`], created by `bnr` functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitsRange {
	pub range: RangeInclusive<u64>,
	pub len: usize,
}
impl MatchBy<BitsRange> for Bits {
	#[inline]
	fn match_by(&self, matcher: BitsRange, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len > self.len {
			MatchSignal::InComplete
		} else {
			let value = bitextract_msb(self.value, self.len, *ind, *ind + matcher.len);
			if matcher.range.contains(&value) {
				*ind += matcher.len;
				MatchSignal::Matched
			} else {
				MatchSignal::MisMatched
			}
		}
	}
}
impl MatchBy<BitsRange> for LBits {
	#[inline]
	fn match_by(&self, matcher: BitsRange, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		if *ind + matcher.len > self.len {
			MatchSignal::InComplete
		} else if matcher.range.contains(&bitextract_lsb(self.value, *ind, *ind + matcher.len)) {
			*ind += matcher.len;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
macro_rules! b_fns {
	($matched:literal, $max_range:literal, [$($n:literal),+]) => {
		$(paste::paste! {
			#[doc = concat!("creates a ", $n, " bit [`Bits`] field.
			
# example
```
assert!(matches!(Bits::new(", $matched, ", ", $n, "): Bits, {b", $n, "(", $matched, ")}));
```")]
			#[inline]
			pub fn [<b $n>](value: u64) -> Bits {
				assert!(value <= u64::MAX >> (64 - $n));
				Bits { value, len: $n }
			}
			#[doc = concat!("creates a ", $n, " bit [`BitsRange`].
			
# example
```
assert!(matches!(Bits::new(", $matched, ", ", $n, "): Bits, {b", $n, "r(0x0..=", $max_range, ")}));
```")]
			#[inline]
			pub fn [<b $n r>](range: RangeInclusive<u64>) -> BitsRange {
				assert!(*range.start() <= u64::MAX >> (64 - $n));
				assert!(*range.end() <= u64::MAX >> (64 - $n));
				BitsRange { range, len: $n }
			}
		})+
	};
}
b_fns!("0x1", "0x1", [1, 2, 3, 4]);
b_fns!("0x12", "0x1F", [5, 6, 7, 8]);
#[rustfmt::skip]
b_fns!("0x123", "0x1FF", [ 
	                                9,  10, 11, 12, 13, 14, 15, 16,
	17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
	33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
	49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64
]);

/// matches a sized byte section by a [`Bits`] [`Matcher`].
///
/// `word` creates a `[u8]` matcher that extract a `size` sized little endian section and matches it against the `bit_matcher` as a `size * 8` bits bitfield.
///
/// # example
/// ```
/// assert!(matches!([1,2,3]: [u8], {word(3, matcher!(for Bits, {b8(3)} {b8(2)} {b8(1)}))}));
/// ```
#[inline]
pub fn word(size: u8, bit_matcher: impl Matcher<Bits>) -> impl Matcher<[u8]> {
	assert!(size <= 8);
	let size = size as usize;
	move |value, ind, status| {
		if *ind + size > value.len() {
			MatchSignal::InComplete
		} else {
			let mut buf = [0u8; 8];
			buf[..size].copy_from_slice(&value[*ind..*ind + size]);
			let bits = Bits { value: u64::from_le_bytes(buf), len: size * 8 };

			let sig = bit_matcher.do_match(&bits, &mut 0, status);
			if sig != MatchSignal::Matched {
				return sig;
			}
			*ind += size;
			MatchSignal::Matched
		}
	}
}
/// like [`word`] but by a [`LBits`] [`Matcher`].
///
/// # example
/// ```
/// assert!(matches!([1,2,3]: [u8], {wordl(3, matcher!(for Bits, {b8(1)} {b8(2)} {b8(3)}))}));
/// ```
#[inline]
pub fn wordl(size: u8, bit_matcher: impl Matcher<LBits>) -> impl Matcher<[u8]> {
	assert!(size <= 8);
	let size = size as usize;
	move |value, ind, status| {
		if *ind + size > value.len() {
			MatchSignal::InComplete
		} else {
			let mut buf = [0u8; 8];
			buf[..size].copy_from_slice(&value[*ind..*ind + size]);
			let bits = LBits { value: u64::from_le_bytes(buf), len: size * 8 };

			let sig = bit_matcher.do_match(&bits, &mut 0, status);
			if sig != MatchSignal::Matched {
				return sig;
			}
			*ind += size;
			MatchSignal::Matched
		}
	}
}

macro_rules! w_macros {
	($d:tt, $($n:literal),*) => {
		paste::paste! {$(
			#[macro_export]
			#[doc(hidden)]
			macro_rules! [<w $n>] {
				($d ($d matcher:tt)+) => {
					$crate::bits::word($n, $crate::matcher!(for $crate::bits::Bits, $d ($d matcher)+))
				}
			}
			#[doc(inline)]
			#[doc = concat!("matches a ", $n, " byte section by a [`Bits`] [grammer expression](crate::docs::gram_ref).
			
this is a shortcut for `word(", $n, ", matcher!(for Bits, ...))`.")]
			pub use [<w $n>];

			#[macro_export]
			#[doc(hidden)]
			macro_rules! [<w $n l>] {
				($d ($d matcher:tt)+) => {
					$crate::bits::wordl($n, $crate::matcher!(for $crate::bits::LBits, $d ($d matcher)+))
				}
			}
			#[doc(inline)]
			#[doc = concat!("matches a ", $n, " byte section by a [`LBits`] [grammer expression](crate::docs::gram_ref).
			
this is a shortcut for `wordl(", $n, ", matcher!(for Bits, ...))`.")]
			pub use [<w $n l>];
		)*}
	};
}
w_macros!($, 1, 2, 3, 4, 5, 6, 7, 8);

/// check if the current index is aligned to `size` bit boundary.
///
/// # example
/// ```
/// assert!(matches!(b8(0x12): Bits, {b4(1)} {aligned(4)} {b4(2)}));
/// ```
pub fn aligned(size: u8) -> impl Matcher<Bits> {
	move |_, ind, _| {
		if *ind % size as usize == 0 { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}
}
/// like [`aligned`] but for [`LBits`]
///
/// # example
/// ```
/// assert!(matches!(LBits::new(8, 0x12)): LBits, {b4(2)} {alignedl(4)} {b4(1)}));
/// ```
pub fn alignedl(size: u8) -> impl Matcher<LBits> {
	move |_, ind, _| {
		if *ind % size as usize == 0 { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}
}
