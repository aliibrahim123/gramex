//! [`str`] matching implementation
//!
//! when `str` feature is enabled, [`str`] matching get enabled and with additional extra [`Matcher`]s through this module.
//!
//! # `str` [`MatchAble`] implementation
//!
//! [`MatchAble`] is implemented for [`str`] where [`Slice`](MatchAble::Slice) is [`&str`](str), [`Token`](MatchAble::Token) is [`char`] and offsets are byte indexes of [`str`].
//!
//! ```
//! assert_eq!(MatchAble::len("abc"), 3);
//! assert_eq!(MatchAble::slice("abc", 1..3), Some("bc"));
//! assert_eq!(MatchAble::slice("abc", 1..4), None);
//! assert_eq!(MatchAble::get_token("abc", 2), Some('c'));
//! assert_eq!(MatchAble::get_token("abc", 3), None);
//! let mut off = 0;
//! assert!(MatchAble::skip_n::<Test>("abc", &mut off, 2).is_ok());
//! assert_eq!(off, 2);
//! ```
//!
//! # core [`Matcher`]s
//!
//! the core [`Matcher`]s for [`str`] are: [`str`], [`char`], [`RangeInclusive<char>`] for range matching, [`String`], and [`Cow<str>`].
//!
//! in addition to `&str`, [`Box<str>`], [`Rc<str>`](alloc_crate::rc::Rc) and [`Arc<str>`](alloc_crate::sync::Arc) that are common to all [`MatchAble`]s.
//!
//! these [`Matcher`]s matches with the [`str`] slice they matched.
//!
//! ```
//! assert_eq!(parse("abc", "abc"), Ok("abc"));
//! assert_eq!(parse("abd", "abc"), Err(MatchError::mismatch("\"abc\"".into(), 0)));
//! assert_eq!(parse("ab", "abc"), Err(MatchError::incomplete("\"abc\"".into(), 0)));
//! assert!(matches("a", 'a'));
//! assert!(matches("a", 'a'..='z'));
//! assert!(matches("abc", String::from("abc")));
//! assert!(matches("abc", Box::new("abc")));
//! ```
//! # pattern [`Matcher`]s
//!
//! the `str` module export multiple [`Matcher`]s that matches characters of common kinds and patterns, like [`alpha`], [`ws`], [`lower`], [`hex`]...
//!
//! ```
//! assert!(matches("a", alpha));
//! assert!(matches("1", num));
//! assert!(matches(" ", ws));
//! assert!(matches!("1aF", hex[3]));
//! ```

use alloc_crate::{borrow::Cow, string::String};
use core::{
	fmt::{Display, Write},
	ops::{Range, RangeInclusive},
};
use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	derive::{define_ref_matcher, define_token_matcher, match_token},
	result::{Expected, MatchError, MatchResult},
	str::matchers::Digit,
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
			M::err(|| MatchError::incomplete(Expected::SomeThing, *off))
		} else {
			*off += self[*off..].len() - chars.as_str().len();
			Ok(M::wrap_success(()))
		}
	}
}

impl Matcher<str> for str {
	type Capture<'src> = &'src str;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src str, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		if matched[*off..].starts_with(self) {
			*off += self.len();
			Ok(M::wrap_success(&matched[*off - self.len()..*off]))
		} else {
			let is_incomplete = *off + self.len() > matched.len();
			M::err(|| MatchError::expected(self.expected(), is_incomplete, *off))
		}
	}
	fn expected(&self) -> Expected {
		Expected::A(wrap_with_quotes(self))
	}
}

/// transform `a` into `"a"`
fn wrap_with_quotes(a: impl Display) -> LeanString {
	let mut str = LeanString::new();
	write!(str, "\"{a}\"").unwrap();
	str
}

define_token_matcher!(char, str, |matcher, char| (
	(char == *matcher).then_some(matcher.len_utf8()),
	Expected::A(wrap_with_quotes(matcher))
));

define_ref_matcher!(String, for str);
define_ref_matcher!(#for('b) Cow<'b, str>, for str);

define_token_matcher!(RangeInclusive<char>, str, |matcher, char| (
	matcher.contains(&char).then_some(char.len_utf8()),
	Expected::Between(wrap_with_quotes(matcher.start()), wrap_with_quotes(matcher.end()),)
));

macro_rules! define_char_patterns {
	[$((
		$name:ident, $matcher:ident, by: |$char:ident| $logic:expr, kind: $kind:literal,
		satisfy: $satisfy:literal,
		$(ascii_equiv: $ascii_equiv:literal, )? $(unicode_equiv: $unicode_equiv:literal, )?
		match_1: $match_1:literal, match_2: $match_2:literal, mismatch: $mismatch:literal
	)),+] => {
		$(
			#[doc = concat!(
				"matches ", $kind, ".\n\n",
				"`", stringify!($name), "` is a [`Matcher`] that matches a character satisfing ",
				$satisfy, ", and capture it as `str` slice.\n\n",
				$("this is unicode aware, for ascii only version, see [`", $ascii_equiv, "`].\n\n",)?
				$("this is ascii only, for unicode aware version, see [`", $unicode_equiv, "`].\n\n",)?
				"# example \n\n```\n",
				"assert_eq!(try_match(\"", $match_1, "\", ", stringify!($name), "), Some(\"",
				$match_1, "\"));\n",
				"assert!(matches(\"", $match_2, "\", ", stringify!($name), "));\n",
				"assert!(!matches(\"", $mismatch, "\", ", stringify!($name), "));\n",
				"assert!(!matches(\"\", ", stringify!($name), "));\n",
				"```"
			)]
			#[allow(nonstandard_style)]
			pub const $name: matchers::$matcher = matchers::$matcher;
		)+

		mod char_pattern_matchers {
			use crate::result::Expected;
			use lean_string::LeanString;
			$(
				#[doc = concat!(
					"[", stringify!($name), "](crate::str::", stringify!($name), ")",
					" [`Matcher`](crate::Matcher).")
				]
				#[derive(Debug, Clone, Copy, PartialEq)]
				#[allow(nonstandard_style)]
				pub struct $matcher;
				crate::derive::define_token_matcher!($matcher, str, |_matcher, $char| (
					$logic.then_some($char.len_utf8()),
					Expected::A(LeanString::from_static_str($kind))
				));
			)+
		}
	};
}

define_char_patterns![
	(
		upper, Upper, by: |char| char.is_uppercase(),
		kind: "an uppercase character", satisfy: "[`char::is_uppercase`]",
		ascii_equiv: "ascii_upper",
		match_1: "A", match_2: "Z", mismatch: "a"
	), (
		lower, Lower, by: |char| char.is_lowercase(),
		kind: "a lowercase character", satisfy: "[`char::is_lowercase`]",
		ascii_equiv: "ascii_lower",
		match_1: "a", match_2: "z", mismatch: "A"
	), (
		num, Num, by: |char| char.is_numeric(),
		kind: "a numeric character", satisfy: "[`char::is_numeric`]",
		ascii_equiv: "dec",
		match_1: "1", match_2: "9", mismatch: "a"
	), (
		alpha, Alpha, by: |char| char.is_alphabetic(),
		kind: "an alphabetic character", satisfy: "[`char::is_alphabetic`]",
		ascii_equiv: "ascii_alpha",
		match_1: "a", match_2: "Z", mismatch: "1"
	), (
		alphanum, Alphanum, by: |char| char.is_alphanumeric(),
		kind: "an alphanumeric character", satisfy: "[`char::is_alphanumeric`]",
		ascii_equiv: "ascii_alphanum",
		match_1: "a", match_2: "9", mismatch: " "
	), (
		ws, Ws, by: |char| char.is_whitespace(),
		kind: "a whitespace character", satisfy: "[`char::is_whitespace`]",
		ascii_equiv: "ascii_ws",
		match_1: " ", match_2: "\t", mismatch: "a"
	), (
		control, Control, by: |char| char.is_control(),
		kind: "a control character", satisfy: "[`char::is_control`]",
		ascii_equiv: "ascii_control",
		match_1: "\n", match_2: "\0", mismatch: "a"
	), (
		ascii, Ascii, by: |char| char.is_ascii(),
		kind: "an ascii character", satisfy: "[`char::is_ascii`]",
		match_1: "a", match_2: "Z", mismatch: "λ"
	), (
		ascii_upper, AsciiUpper, by: |char| char.is_ascii_uppercase(),
		kind: "an ascii uppercase character", satisfy: "[`char::is_ascii_uppercase`]",
		unicode_equiv: "upper",
		match_1: "A", match_2: "Z", mismatch: "a"
	), (
		ascii_lower, AsciiLower, by: |char| char.is_ascii_lowercase(),
		kind: "an ascii lowercase character", satisfy: "[`char::is_ascii_lowercase`]",
		unicode_equiv: "lower",
		match_1: "a", match_2: "z", mismatch: "A"
	), (
		ascii_alpha, AsciiAlpha, by: |char| char.is_ascii_alphabetic(),
		kind: "an ascii alphabetic character", satisfy: "[`char::is_ascii_alphabetic`]",
		unicode_equiv: "alpha",
		match_1: "a", match_2: "Z", mismatch: "1"
	), (
		ascii_alphanum, AsciiAlphanum, by: |char| char.is_ascii_alphanumeric(),
		kind: "an ascii alphanumeric character", satisfy: "[`char::is_ascii_alphanumeric`]",
		unicode_equiv: "alphanum",
		match_1: "a", match_2: "9", mismatch: " "
	), (
		ascii_ws, AsciiWs, by: |char| char.is_ascii_whitespace(),
		kind: "an ascii whitespace character", satisfy: "[`char::is_ascii_whitespace`]",
		unicode_equiv: "ws",
		match_1: " ", match_2: "\t", mismatch: "a"
	), (
		ascii_control, AsciiControl, by: |char| char.is_ascii_control(),
		kind: "an ascii control character", satisfy: "[`char::is_ascii_control`]",
		unicode_equiv: "control",
		match_1: "\n", match_2: "\0", mismatch: "a"
	), (
		ascii_printable, AsciiPrintable, by: |char| char.is_ascii_graphic(),
		kind: "an ascii printable character", satisfy: "[`char::is_ascii_graphic`]",
		match_1: "a", match_2: "!", mismatch: " "
	), (
		ascii_punct, AsciiPunct, by: |char| char.is_ascii_punctuation(),
		kind: "an ascii punctuation character", satisfy: "[`char::is_ascii_punctuation`]",
		match_1: "!", match_2: "#", mismatch: "a"
	), (
		dec, Dec, by: |char| matches!(char, '0'..='9'),
		kind: "a decimal digit", satisfy: "`'0'..'9'",
		match_1: "0", match_2: "9", mismatch: "a"
	), (
		hex, Hex, by: |char| matches!(char, '0'..='9' | 'a'..='f' | 'A'..='F'),
		kind: "a hexadecimal digit", satisfy: "`'0'..'9' | 'a'..'f' | 'A'..'F'`",
		match_1: "9", match_2: "f", mismatch: "g"
	), (
		hex_lower, HexLower, by: |char| matches!(char, '0'..='9' | 'a'..='f'),
		kind: "a lower hexadecimal digit", satisfy: "`'0'..'9' | 'a'..'f'`",
		match_1: "9", match_2: "f", mismatch: "F"
	), (
		hex_upper, HexUpper, by: |char| matches!(char, '0'..='9' | 'A'..='F'),
		kind: "an upper hexadecimal digit", satisfy: "`'0'..'9' | 'A'..'F'`",
		match_1: "9", match_2: "F", mismatch: "f"
	), (
		bin, Bin, by: |char| matches!(char, '0' | '1'),
		kind: "a binary digit", satisfy: "`'0' | '1'`",
		match_1: "0", match_2: "1", mismatch: "2"
	), (
		octal, Octal, by: |char| matches!(char, '0'..='7'),
		kind: "an octal digit", satisfy: "`'0'..'7'`",
		match_1: "0", match_2: "7", mismatch: "8"
	)
];

/// matches a digit character of given `radix`.
///
/// `digit` create a [`Matcher`] that matches a character that is a digit of `radix` between `2` and `36` inclusive.
///
/// it is based on [`char::is_digit`], and capture the character `str` slice.
///
/// # example
/// ```
/// assert_eq!(try_match("1", digit(10)), Some("1"));
/// assert!(!matches("g", digit(10)));
/// assert!(matches("g", digit(20)));
/// assert!(!matches("+", digit(36)));
/// ```
pub fn digit(radix: u8) -> Digit {
	assert!(radix >= 2 && radix <= 36);
	Digit(radix)
}

/// [str](crate::str) items [`Matcher`]s
pub mod matchers {
	use core::fmt::Write;
	use lean_string::LeanString;

	use crate::{
		Matcher, Mode,
		derive::match_token,
		result::{Expected, MatchResult},
	};

	#[doc(inline)]
	pub use super::char_pattern_matchers::*;

	/// [`digit`](crate::str::digit) [`Matcher`].
	pub struct Digit(pub(crate) u8);
	impl Matcher<str> for Digit {
		type Capture<'src> = &'src str;
		fn do_match<'src, M: Mode>(
			&self, matched: &'src str, off: &mut usize,
		) -> MatchResult<&'src str, M> {
			let matcher =
				|char: char| char.is_digit(self.0 as u32).then_some(char.len_utf8());
			match_token::<M, _>(matched, off, matcher, || self.expected())
		}
		fn expected(&self) -> Expected {
			let mut buf = LeanString::new();
			write!(buf, "a base-{} digit", self.0).unwrap();
			Expected::A(buf)
		}
	}
}
