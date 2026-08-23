use std::{borrow::Cow, ops::RangeInclusive, rc::Rc, sync::Arc};

use crate::{Capturer, Expected, MatchAble, Matcher, derive_check_from_test, derive_slice_capture};

impl MatchAble for str {
	type Slice<'a> = &'a str;
	type Offset = usize;

	fn len(&self) -> Self::Offset {
		self.len()
	}
	fn slice<'a>(&'a self, range: std::ops::Range<usize>) -> Option<Self::Slice<'a>> {
		self.get(range)
	}
	fn skip_n(&self, off: &mut Self::Offset, n: usize) -> bool {
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

impl Matcher<str> for str {
	fn test(&self, matched: &str, off: &mut usize) -> bool {
		let Some(section) = matched.get(*off..) else {
			*off = matched.len();
			return false;
		};
		let res = section.starts_with(self);
		*off += if res { self.len() } else { 0 };
		res
	}
	derive_check_from_test!(str, |_self| Expected::A(format!("\"{_self}\"").into()));
}
impl Capturer<str> for str {
	derive_slice_capture!(str);
}

impl Matcher<str> for char {
	fn test(&self, matched: &str, off: &mut usize) -> bool {
		let Some(char) = matched[*off..].chars().next() else {
			*off = matched.len();
			return false;
		};
		let cond = char == *self;
		*off += if cond { char.len_utf8() } else { 0 };
		cond
	}
	derive_check_from_test!(str, |_self| Expected::A(format!("\"{_self}\"").into()));
}

macro_rules! impl_ref {
	[$($T:ty),+] => {
		$(impl Matcher<str> for $T {
			fn test(&self, matched: &str, off: &mut usize) -> bool {
				AsRef::<str>::as_ref(self).test(matched, off)
			}
			fn check(&self, matched: &str, off: &mut usize) -> crate::MatchResult<(), str> {
				AsRef::<str>::as_ref(self).check(matched, off)
			}
		}
		impl Capturer<str> for $T {
			derive_slice_capture!(str);
		})+
	};
}
impl_ref![String, Box<str>, Rc<str>, Arc<str>];
impl<'a> Matcher<str> for Cow<'a, str> {
	fn test(&self, matched: &str, off: &mut usize) -> bool {
		AsRef::<str>::as_ref(self).test(matched, off)
	}
	fn check(&self, matched: &str, off: &mut usize) -> crate::MatchResult<(), str> {
		AsRef::<str>::as_ref(self).check(matched, off)
	}
}
impl<'b> Capturer<str> for Cow<'b, str> {
	derive_slice_capture!(str);
}

impl Matcher<str> for RangeInclusive<char> {
	fn test(&self, matched: &str, off: &mut usize) -> bool {
		let Some(char) = matched[*off..].chars().next() else {
			*off = matched.len();
			return false;
		};
		let cond = self.contains(&char);
		*off += if cond { char.len_utf8() } else { 0 };
		cond
	}
	derive_check_from_test!(str, |_self| Expected::Between(
		format!("{}", _self.start()).into(),
		format!("{}", _self.end()).into()
	));
}
impl Capturer<str> for RangeInclusive<char> {
	derive_slice_capture!(str);
}
