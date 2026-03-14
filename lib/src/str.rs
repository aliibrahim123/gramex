//! # [`str`] implementation
//! [`str`] implements [`MatchAble`] where the tokens are the characters in the string, index by byte position.
//!
//! [`str`] implements [`MatchBy`] for [`char`], [`&str`](str), [`&char`](char), [`&String`](String), [`&Box<str>`](Box<str>), and [`&Cow<'a, str>`](Cow<str>).
//!
//! it also implements range matching for [`char`].
//!
//! # example
//! ```
//! assert!(matches!("abc": str, 'a' 'b' 'c'));
//! assert!(matches!("abc": str, "abc"));
//! assert!(matches!("abc": str, 'a'..'z' {&String::from("b
//! c")}));
//! ```

use std::{borrow::Cow, ops::RangeInclusive};

use crate::{MatchAble, MatchBy, MatchSignal, MatchStatus, Matcher};

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

macro_rules! match_char {
	($value:ident, $ind:ident, $char:ident => $predicate:expr) => {{
		let Some($char) = $value[*$ind..].chars().next() else {
			return MatchSignal::InComplete;
		};
		*$ind += $char.len_utf8();
		if $predicate { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}};
}

/// matches a char using a predicate.
///
/// like [`a`](crate::a), but accept a char predicate, not a 1 char [`str`] slice.
///
/// # example
/// ```
/// assert!(matches!("abc": str, {a_char(char::is_lowercase)}[3]));
/// ```
#[inline]
pub fn a_char(predicate: impl Fn(char) -> bool) -> impl Matcher<str> {
	move |v, ind, _| match_char!(v, ind, char => predicate(char))
}

/// matches a char that is lowercase.
///
/// this is a unicode aware version of [`ascii_lower`].
///
/// it is based on [`char::is_lowercase`].
///
/// # example
/// ```
/// assert!(matches!("abc": str, lower[3]));
/// ```
#[inline]
pub fn lower(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_lowercase())
}
/// matches a char that is uppercase.
///
/// this is a unicode aware version of [`ascii_upper`].
///
/// it is based on [`char::is_uppercase`].
///
/// # example
/// ```
/// assert!(matches!("ABC": str, upper[3]));
/// ```
#[inline]
pub fn upper(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_uppercase())
}
/// matches a char that is alphabetic.
///
/// this is a unicode aware version of [`ascii_alpha`].
///
/// it is based on [`char::is_alphabetic`].
///
/// # example
/// ```
/// assert!(matches!("abc": str, alpha[3]));
/// ```
#[inline]
pub fn alpha(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_alphabetic())
}
/// matches a char that is numeric.
///
/// this function is unicode aware.
///
/// it is based on [`char::is_numeric`].
///
/// # example
/// ```
/// assert!(matches!("123": str, num[3]));
/// ```
#[inline]
pub fn num(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_numeric())
}
/// matches a char that is alphanumeric.
///
/// this is a unicode aware version of [`ascii_alphanum`].
///
/// it is based on [`char::is_alphanumeric`].
///
/// # example
/// ```
/// assert!(matches!("abc123": str, alphanum[6]));
/// ```
#[inline]
pub fn alphanum(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_alphanumeric())
}
/// matches a char that is a whitespace.
///
/// this is a unicode aware version of [`ascii_ws`].
///
/// it is based on [`char::is_whitespace`].
///
/// # example
/// ```
/// assert!(matches!(" \n\t": str, ws[3]));
/// ```
#[inline]
pub fn ws(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_whitespace())
}
/// matches a char that is a control character.
///
/// this is a unicode aware version of [`ascii_control`].
///
/// it is based on [`char::is_control`].
///
/// # example
/// ```
/// assert!(matches!("\n": str, control));
/// ```
#[inline]
pub fn control(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_control())
}

/// matches a char that is ascii.
///
/// it is based on [`char::is_ascii`].
///
/// # example
/// ```
/// assert!(matches!("abc": str, ascii[3]));
/// ```
#[inline]
pub fn ascii(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii())
}
/// matches a char that is ascii lowercase.
///
/// it is based on [`char::is_ascii_lowercase`].
///
/// # example
/// ```
/// assert!(matches!("abc": str, ascii_lower[3]));
/// ```
#[inline]
pub fn ascii_lower(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_lowercase())
}
/// matches a char that is ascii uppercase.
///
/// it is based on [`char::is_ascii_uppercase`].
///
/// # example
/// ```
/// assert!(matches!("ABC": str, ascii_upper[3]));
/// ```
#[inline]
pub fn ascii_upper(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_uppercase())
}
/// matches a char that is ascii alphabetic.
///
/// it is based on [`char::is_ascii_alphabetic`].
///
/// # example
/// ```
/// assert!(matches!("abc": str, ascii_alpha[3]));
/// ```
#[inline]
pub fn ascii_alpha(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_alphabetic())
}
/// matches a char that is ascii alphanumeric.
///
/// it is based on [`char::is_ascii_alphanumeric`].
///
/// # example
/// ```
/// assert!(matches!("abc123": str, ascii_alphanum[6]));
/// ```
#[inline]
pub fn ascii_alphanum(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_alphanumeric())
}
/// matches a char that is an ascii whitespace.
///
/// it is based on [`char::is_ascii_whitespace`].
///
/// # example
/// ```
/// assert!(matches!(" \n\t": str, ascii_ws[3]));
/// ```
#[inline]
pub fn ascii_ws(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_whitespace())
}
/// matches a char that is an ascii control character.
///
/// it is based on [`char::is_ascii_control`].
///
/// # example
/// ```
/// assert!(matches!("\n": str, ascii_control));
/// ```
#[inline]
pub fn ascii_control(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_control())
}
/// matches a char that is ascii printable.
///
/// it is based on [`char::is_ascii_graphic`].
///
/// # example
/// ```
/// assert!(matches!("a1!": str, ascii_printable[3]));
/// ```
#[inline]
pub fn ascii_printable(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_graphic())
}
/// matches a char that is ascii punctuation.
///
/// it is based on [`char::is_ascii_punctuation`].
///
/// # example
/// ```
/// assert!(matches!("!@#": str, ascii_punct[3]));
/// ```
#[inline]
pub fn ascii_punct(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => char.is_ascii_punctuation())
}

/// matches a char that is a decimal digit (`0`-`9`).
///
/// # example
/// ```
/// assert!(matches!("123": str, dec[3]));
/// ```
#[inline]
pub fn dec(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0'..='9'))
}
/// matches a char that is a hexadecimal digit.
///
/// it matches `0`-`9`, `a`-`f`, and `A`-`F`.
///
/// # example
/// ```
/// assert!(matches!("1aF": str, hex[3]));
/// ```
#[inline]
pub fn hex(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0'..='9' | 'a'..='f' | 'A'..='F'))
}
/// matches a char that is a lower hexadecimal digit.
///
/// it matches `0`-`9`, and `a`-`f`.
///
/// # example
/// ```
/// assert!(matches!("1af": str, hex_lower[3]));
/// ```
#[inline]
pub fn hex_lower(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0'..='9' | 'a'..='f'))
}
/// matches a char that is an upper hexadecimal digit.
///
/// it matches `0`-`9`, and `A`-`F`.
///
/// # example
/// ```
/// assert!(matches!("1AF": str, hex_upper[3]));
/// ```
#[inline]
pub fn hex_upper(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0'..='9' | 'A'..='F'))
}
/// matches a char that is an octal digit (`0`-`7`).
///
/// # example
/// ```
/// assert!(matches!("123": str, octal[3]));
/// ```
#[inline]
pub fn octal(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0'..='7'))
}
/// matches a char that is a binary digit (`0` or `1`).
///
/// # example
/// ```
/// assert!(matches!("01": str, bin[2]));
/// ```
#[inline]
pub fn bin(value: &str, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	_ = status;
	match_char!(value, ind, char => matches!(char, '0' | '1'))
}
/// matches a char that is a digit of a given radix.
///
/// it is based on [`char::is_digit`].
///
/// # example
/// ```
/// assert!(matches!("123": str, {digit(4)}[3]));
/// ```
#[inline]
pub fn digit(radix: u32) -> impl Matcher<str> {
	move |value, ind, _| match_char!(value, ind, char => char.is_digit(radix))
}
