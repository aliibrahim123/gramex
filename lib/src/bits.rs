use core::{
	fmt::{self, Binary, Formatter, Write},
	ops::{Range, RangeInclusive},
};

use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	core::{define_slice_matcher, define_token_matcher, match_slice, match_token},
	result::{Expected, MatchResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bits {
	pub value: u64,
	pub len: u8,
}
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

impl<const N: usize> From<[u8; N]> for Bits {
	#[inline]
	fn from(value: [u8; N]) -> Self {
		assert!(N <= 8);
		let mut buf = [0u8; 8];
		buf[..N].copy_from_slice(&value);
		Self { value: u64::from_le_bytes(buf), len: (N * 8) as u8 }
	}
}
impl<const N: usize> TryFrom<Bits> for [u8; N] {
	type Error = ();
	#[inline]
	fn try_from(bits: Bits) -> Result<Self, Self::Error> {
		assert!(N <= 8);
		if bits.len > (N * 8) as u8 {
			return Err(());
		}
		let mut buf = [0u8; N];
		buf.copy_from_slice(&bits.value.to_le_bytes()[..N]);
		Ok(buf)
	}
}
impl TryFrom<&[u8]> for Bits {
	type Error = ();
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		if value.len() > 8 {
			return Err(());
		}
		let mut buf = [0u8; 8];
		buf[..value.len()].copy_from_slice(value);
		Ok(Self { value: u64::from_le_bytes(buf), len: value.len() as u8 * 8 })
	}
}

impl Binary for Bits {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "0b{:01$b}", self.value, self.len as usize)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LBits(pub Bits);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BBits(pub Bits);

impl LBits {
	#[inline]
	pub fn new(len: u8, value: u64) -> LBits {
		LBits(b(len, value))
	}
}
impl BBits {
	#[inline]
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
steal_bits_from![
	u8, u16, u32, i8, i16, i32,
	usize, isize, bool, f32, f64,
	#for (const N: usize) [u8; N]
];

impl TryFrom<&[u8]> for LBits {
	type Error = ();
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		Ok(LBits(Bits::try_from(value)?))
	}
}
impl TryFrom<&[u8]> for BBits {
	type Error = ();
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		Ok(BBits(Bits::try_from(value)?))
	}
}

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

#[inline]
fn bit_extract_le(value: u64, start: u8, end: u8) -> u64 {
	if start == end {
		return 0;
	}
	(value >> start) & (u64::MAX >> (64 - (end - start)))
}
#[inline]
fn bit_extract_be(value: u64, start: u8, end: u8, len: u8) -> u64 {
	bit_extract_le(value, len - end, len - start)
}

fn to_bin(value: u64, len: u8) -> LeanString {
	let mut str = LeanString::new();
	write!(str, "0b{value:00$b}", len as usize).unwrap();
	str
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitRange {
	pub start: u64,
	pub end: u64,
	pub len: u8,
}
impl BitRange {
	#[inline]
	fn len(&self) -> usize {
		self.len as usize
	}
}
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
			u8, u16, u32, i8, i16, i32,
			#for (const N:usize) [u8; N]
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

pub fn a_b<F: Fn(u64) -> bool>(len: u8, fun: F) -> A<F> {
	assert!(len <= 64 && len != 0);
	A { fun, len }
}
#[derive(Debug, Clone, Copy)]
pub struct A<F> {
	fun: F,
	len: u8,
}
