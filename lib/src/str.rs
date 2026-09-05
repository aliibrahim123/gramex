use alloc_crate::{borrow::Cow, boxed::Box, rc::Rc, string::String, sync::Arc};
use core::{
	fmt::{Display, Write},
	ops::{Range, RangeInclusive},
};
use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	result::{Expected, MatchError, MatchResult},
};

impl MatchAble for str {
	type Token<'src> = char;
	type Slice<'src> = &'src str;

	#[inline]
	fn len(&self) -> usize {
		self.len()
	}
	#[inline]
	fn get_token<'src>(&'src self, off: usize) -> Option<char> {
		self[off..].chars().next()
	}
	#[inline]
	fn slice<'src>(&'src self, range: Range<usize>) -> Option<&'src str> {
		self.get(range)
	}
	#[inline]
	fn skip_n<M: Mode>(&self, off: &mut usize, n: usize) -> MatchResult<(), M> {
		if n == 0 {
			return Ok(M::wrap_success(()));
		}
		let mut chars = self[*off..].chars();
		if chars.nth(n - 1).is_none() {
			*off = self.len();
			M::err(|| MatchError::incomplete(Expected::SomeThing, *off))
		} else {
			*off += self[*off..].len() - chars.as_str().len();
			Ok(M::wrap_success(()))
		}
	}
}

macro_rules! define_matcher {
	($ty:ty, ($matcher:ident, $rem:ident) => $logic:expr, $expected:expr) => {
		impl Matcher<str> for $ty {
			type Capture<'src> = &'src str;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src str, off: &mut usize,
			) -> MatchResult<&'src str, M> {
				let $rem = &matched[*off..];
				let $matcher = self;
				let (res, len) = $logic;
				if res {
					*off += len;
					M::ok(|| &$rem[..len])
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

fn wrap_with_quotes(a: impl Display) -> LeanString {
	let mut str = LeanString::new();
	write!(str, "\"{a}\"").unwrap();
	str
}

define_matcher!(str,
	(matcher, rem) => (rem.starts_with(matcher), matcher.len()),
	Expected::A(wrap_with_quotes(matcher))
);

define_matcher!(char,
	(matcher, rem) => (rem.starts_with(*matcher), matcher.len_utf8()),
	Expected::A(wrap_with_quotes(matcher))
);

macro_rules! impl_ref {
	[$($(#for <$life:lifetime>)? $T:ty),+] => {
		$(impl$(<$life>)? Matcher<str> for $T {
			type Capture<'src> = &'src str;
			#[inline]
			fn do_match<'src, M: Mode>(
				&self, matched: &'src str, off: &mut usize,
			) -> MatchResult<Self::Capture<'src>, M> {
				AsRef::<str>::as_ref(self).do_match::<M>(matched, off)
			}
			fn expected(&self) -> Expected {
				AsRef::<str>::as_ref(self).expected()
			}
		})+
	};
}
impl_ref![String, Box<str>, Rc<str>, Arc<str>, #for<'b> Cow<'b, str>];

define_matcher!(RangeInclusive<char>,
	(matcher, rem) => match rem.chars().next() {
		Some(c) => (matcher.contains(&c), c.len_utf8()),
		_ => (false, 0),
	},
	Expected::Between(
		wrap_with_quotes(matcher.start()),
		wrap_with_quotes(matcher.end()),
	)
);
