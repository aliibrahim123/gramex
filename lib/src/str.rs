use std::{borrow::Cow, ops::RangeInclusive};

use crate::{MatchAble, MatchBy, MatchSignal, MatchStatus};

impl MatchAble for str {
	type Slice<'a> = &'a str;
	#[inline]
	fn len(&self) -> usize {
		self.len()
	}
	#[inline]
	fn slice(&self, range: std::ops::Range<usize>) -> &str {
		&self[range]
	}
	#[inline]
	fn get_n(&self, ind: &mut usize, n: usize, _status: &MatchStatus) -> Result<&str, MatchSignal> {
		let mut chars = self[*ind..].char_indices();
		let start = *ind;
		// ensure the end char
		if chars.nth(n - 1).is_none() {
			return Err(MatchSignal::InComplete);
		}

		let end = chars.next().map(|(i, _)| i + start).unwrap_or(self.len());
		*ind = end;
		Ok(&self[start..end])
	}
}

impl MatchBy<char> for str {
	#[inline]
	fn match_by(&self, matcher: char, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		let len = matcher.len_utf8();
		if len + *ind > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(matcher) {
			*ind += len;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a> MatchBy<&'a str> for str {
	#[inline]
	fn match_by(&self, matcher: &'a str, ind: &mut usize, _status: &MatchStatus) -> MatchSignal {
		let len = matcher.len();
		if len + *ind > self.len() {
			MatchSignal::InComplete
		} else if self[*ind..].starts_with(matcher) {
			*ind += len;
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
impl<'a> MatchBy<&'a char> for str {
	#[inline]
	fn match_by(&self, matcher: &'a char, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
		MatchBy::match_by(self, *matcher, ind, status)
	}
}
macro_rules! as_ref_impl {
	[$($ty:ty) +] => {
		$(impl<'a> MatchBy<&'a $ty> for str {
			#[inline]
			fn match_by(
				&self, matcher: &'a $ty, ind: &mut usize, status: &MatchStatus,
			) -> MatchSignal {
				MatchBy::match_by(self, AsRef::<str>::as_ref(matcher), ind, status)
			}
		})*
	};
}
as_ref_impl![String Box<str> Cow<'a, str>];
impl MatchBy<RangeInclusive<char>> for str {
	#[inline]
	fn match_by(
		&self, matcher: RangeInclusive<char>, ind: &mut usize, _status: &MatchStatus,
	) -> MatchSignal {
		let Some(cur_char) = self[*ind..].chars().next() else {
			return MatchSignal::InComplete;
		};
		if matcher.contains(&cur_char) {
			*ind += cur_char.len_utf8();
			MatchSignal::Matched
		} else {
			MatchSignal::MisMatched
		}
	}
}
