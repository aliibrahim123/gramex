use core::{
	fmt::{Display, Write},
	ops::{Range, RangeInclusive},
};

use alloc_crate::{rc::Rc, sync::Arc};
use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
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

macro_rules! define_matcher {
	($ty:ty, |$matcher:ident, $rem:ident| $logic:expr, $expected:expr) => {
		impl Matcher<[u8]> for $ty {
			type Capture<'src> = &'src [u8];
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src [u8], off: &mut usize,
			) -> MatchResult<&'src [u8], M> {
				let $matcher = self;
				let $rem = &matched[*off..];
				let (res, len) = $logic;
				if res {
					*off += len;
					Ok(M::wrap_success(&$rem[..len]))
				} else if len > $rem.len() {
					*off = matched.len();
					M::err(|| MatchError::incomplete(self.expected(), *off))
				} else {
					M::err(|| MatchError::mismatch(self.expected(), *off))
				}
			}
			fn expected(&self) -> Expected {
				let $matcher = self;
				$expected
			}
		}
	};
}

define_matcher!(
	u8,
	|matcher, rem| (rem.get(0).is_some_and(|v| v == matcher), 1),
	Expected::A(to_hex(&[*matcher]))
);
define_matcher!(
	[u8],
	|matcher, rem| (rem.starts_with(matcher), matcher.len()),
	Expected::A(to_hex(matcher))
);
define_matcher!(
	RangeInclusive<u8>,
	|matcher, rem| (rem.get(0).is_some_and(|v| matcher.contains(v)), 1),
	Expected::Between(to_hex(&[*matcher.start()]), to_hex(&[*matcher.end()]))
);

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

macro_rules! define_ref_matcher {
	[$($(#for<const $N:ident>)? $ty:ty),+] => {
		$(impl $(<const $N: usize>)? Matcher<[u8]> for $ty {
			type Capture<'src> = &'src [u8];
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src [u8], off: &mut usize,
			) -> MatchResult<&'src [u8], M> {
				AsRef::<[u8]>::as_ref(self).do_match::<M>(matched, off)
			}
			fn expected(&self) -> Expected {
				AsRef::<[u8]>::as_ref(self).expected()
			}
		})+
	};
}
define_ref_matcher![Vec<u8>, #for<const N> [u8; N]];

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
