use core::{
	fmt::Write,
	marker::PhantomData,
	mem::size_of,
	ops::{Range, RangeInclusive},
};

use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	core::{
		define_ref_matcher, define_slice_matcher, define_token_matcher, match_slice,
		match_token,
	},
	result::{Expected, MatchError, MatchResult},
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

pub fn aligned(align: usize) -> Aligned {
	Aligned { align }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aligned {
	align: usize,
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

pub trait AsLEBytes {}
pub fn le<T: AsLEBytes>(value: T) -> LE<T> {
	LE(value)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LE<T>(T);

pub trait AsBEBytes {}
pub fn be<T: AsBEBytes>(value: T) -> BE<T> {
	BE(value)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BE<T>(T);

pub fn a_le<T: AsLEBytes, F: Fn(T) -> bool>(pred: F) -> ALE<T, F> {
	ALE(pred, PhantomData)
}
#[derive(Debug, Clone, Copy)]
pub struct ALE<T, F>(F, PhantomData<T>);

pub fn a_be<T: AsBEBytes, F: Fn(T) -> bool>(pred: F) -> ABE<T, F> {
	ABE(pred, PhantomData)
}
#[derive(Debug, Clone, Copy)]
pub struct ABE<T, F>(F, PhantomData<T>);

fn expected_t_value(ty: &'static str) -> Expected {
	let mut str = LeanString::new();
	write!(str, "a {ty} value").unwrap();
	Expected::A(str)
}

macro_rules! define_nb_matcher {
	[$(($ty:ident, $le:ident, $be:ident)),+] => {
		$(
			define_nb_matcher!(define(tn_xe, $le, $ty, from_le_bytes));
			define_nb_matcher!(define(tn_xe, $be, $ty, from_be_bytes));
			impl AsLEBytes for $ty {}
			impl AsBEBytes for $ty {}
			define_nb_matcher!(define(XE<tn>, LE, $ty, to_le_bytes));
			define_nb_matcher!(define(XE<tn>, BE, $ty, to_be_bytes));
			define_nb_matcher!(define(a_tn_xe, ALE, $ty, from_le_bytes));
			define_nb_matcher!(define(a_tn_xe, ABE, $ty, from_be_bytes));
		)+
	};
	(define(tn_xe, $name:ident, $ty:ident, $from_bytes:ident)) => {
		#[allow(nonstandard_style)]
		#[derive(Debug, Clone, Copy, PartialEq)]
		pub struct $name;
		impl Matcher<[u8]> for $name {
			type Capture<'src> = $ty;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src [u8], off: &mut usize,
			) -> MatchResult<$ty, M> {
				match_slice::<M, _, _>(matched, off, size_of::<$ty>(), |bytes| {
					Some($ty::$from_bytes(bytes.try_into().unwrap()))
				}, || self.expected())
			}
			fn expected(&self) -> Expected {
				expected_t_value(stringify!($ty))
			}
		}
	};
	(define(XE<tn>, $XE:ident, $ty:ident, $to_bytes:ident)) => {
		impl Matcher<[u8]> for $XE<$ty> {
			type Capture<'src> = &'src [u8];
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src [u8], off: &mut usize,
			) -> MatchResult<&'src [u8], M> {
				self.0.$to_bytes().do_match::<M>(matched, off)
			}
			fn expected(&self) -> Expected {
				<_ as Matcher<[u8]>>::expected(&self.0.$to_bytes())
			}
		}
	};
	(define(a_tn_xe, $AXE:ident, $ty:ident, $from_bytes:ident)) => {
		impl<F: Fn($ty) -> bool> Matcher<[u8]> for $AXE<$ty, F> {
			type Capture<'src> = $ty;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src [u8], off: &mut usize,
			) -> MatchResult<$ty, M> {
				match_slice::<M, _, _>(matched, off, size_of::<$ty>(), |bytes| {
					let value = $ty::$from_bytes(bytes.try_into().unwrap());
					self.0(value).then_some(value)
				}, || self.expected())
			}
			fn expected(&self) -> Expected {
				expected_t_value(stringify!($ty))
			}
		}
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
	(f64, f64_le, f64_be)
];

#[cfg(feature = "bits")]
mod bits_ext {
	use core::fmt::Write;
	use lean_string::LeanString;

	use crate::{
		Matcher, Mode,
		bits::{BBits, LBits},
		result::{MatchError, MatchResult},
	};

	macro_rules! word_xe {
		($word_xe:ident, $XBits:ident, $WordXE:ident) => {
			pub fn $word_xe<M: Matcher<$XBits>>(bytes: u8, matcher: M) -> $WordXE<M> {
				assert!(bytes <= 8 && bytes != 0);
				$WordXE { bytes, matcher }
			}
			#[derive(Debug, Clone, Copy, PartialEq)]
			pub struct $WordXE<U> {
				bytes: u8,
				matcher: U,
			}
			impl<U, C> Matcher<[u8]> for $WordXE<U>
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
					let bits = $XBits::try_from(slice).unwrap();
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
	word_xe!(word_le, LBits, WordLE);
	word_xe!(word_be, BBits, WordBE);
}
#[cfg(feature = "bits")]
pub use bits_ext::*;
