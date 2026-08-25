use std::{borrow::Cow, ops::RangeInclusive, rc::Rc, sync::Arc};

use crate::{
	MatchAble, MatchResult, Matcher, Mode,
	result::{Expected, MatchError},
};

impl MatchAble for str {
	type Slice<'a> = &'a str;

	fn len(&self) -> usize {
		self.len()
	}
	fn slice<'a>(&'a self, range: std::ops::Range<usize>) -> Option<Self::Slice<'a>> {
		self.get(range)
	}
	fn skip_n(&self, off: &mut usize, n: usize) -> bool {
		let mut chars = self[*off..].chars();
		if chars.nth(n - 1).is_none() {
			*off = self.len();
			false
		} else {
			*off += self[*off..].len() - chars.as_str().len();
			true
		}
	}
}

macro_rules! define_matcher {
	($ty:ty, ($matcher:ident, $rem:ident) => $logic:expr, $expected:expr) => {
		impl Matcher<str> for $ty {
			type Capture<'a> = &'a str;
			fn do_match<'a, M: Mode>(
				&self, matched: &'a str, off: &mut usize,
			) -> MatchResult<Self::Capture<'a>, M> {
				let $rem = &matched[*off..];
				let $matcher = self;
				let (res, len) = $logic;
				if res {
					*off += len;
					M::ok(|| &$rem[..len])
				} else if len > $rem.len() {
					*off = matched.len();
					M::err(|| MatchError::incomplete($expected, *off))
				} else {
					M::err(|| MatchError::mismatch($expected, *off))
				}
			}
		}
	};
}

define_matcher!(str,
	(matcher, rem) => (rem.starts_with(matcher), matcher.len()),
	Expected::A(format!("\"{matcher}\"").into())
);

define_matcher!(char,
	(matcher, rem) => (rem.starts_with(*matcher), matcher.len_utf8()),
	Expected::A(format!("\"{matcher}\"").into())
);

macro_rules! impl_ref {
	[$($(#for <$life:lifetime>)? $T:ty),+] => {
		$(impl$(<$life>)? Matcher<str> for $T {
			type Capture<'a> = &'a str;
			fn do_match<'a, M: Mode>(
				&self, matched: &'a str, off: &mut usize,
			) -> MatchResult<Self::Capture<'a>, M> {
				AsRef::<str>::as_ref(self).do_match::<M>(matched, off)
			}
		})+
	};
}
impl_ref![String, Box<str>, Rc<str>, Arc<str>, #for<'b> Cow<'b, str>];

define_matcher!(RangeInclusive<char>,
	(matcher, rem) => match rem.chars().next() {
		Some(c) => (matcher.contains(&c), c.len_utf8()),
		_ => (false, 4),
	},
	Expected::Between(
		format!("{}", matcher.start()).into(),
		format!("{}", matcher.end()).into()
	)
);
