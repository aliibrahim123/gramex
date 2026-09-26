//! [`str`] matching implementation
//!
//! when `str` feature is enabled, [`str`] matching get enabled and with additional extra [`Matcher`]s through this module.
//!
//! # `str` [`MatchAble`] implementation
//!
//! [`MatchAble`] is implemented for [`str`] where [`Slice`](MatchAble::Slice) is [`&str`](str), [`Token`](MatchAble::Token) is [`char`](char) and offsets are byte indexes of [`str`].
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

macro_rules! define_pattern_matcher {
	($name:ident, $matcher:ident, |$char:ident| $logic:expr, $kind:literal) => {
		#[derive(Debug, Clone, Copy, PartialEq)]
		#[doc(hidden)]
		#[allow(nonstandard_style)]
		pub struct $matcher;
		#[allow(nonstandard_style)]
		pub const $name: $matcher = $matcher;
		define_token_matcher!($matcher, str, |_matcher, $char| (
			$logic.then_some($char.len_utf8()),
			Expected::A(LeanString::from_static_str($kind))
		));
	};
}

define_pattern_matcher!(
	upper,
	upper_M,
	|char| char.is_uppercase(),
	"an uppercase character"
);
define_pattern_matcher!(
	lower,
	lower_M,
	|char| char.is_lowercase(),
	"a lowercase character"
);
define_pattern_matcher!(
	alpha,
	alpha_M,
	|char| char.is_alphabetic(),
	"an alphabetic character"
);
define_pattern_matcher!(num, num_M, |char| char.is_numeric(), "a numeric character");
define_pattern_matcher!(
	alphanum,
	alphanum_M,
	|char| char.is_alphanumeric(),
	"an alphanumeric character"
);
define_pattern_matcher!(ws, ws_M, |char| char.is_whitespace(), "a whitespace character");
define_pattern_matcher!(
	control,
	control_M,
	|char| char.is_control(),
	"a control character"
);
define_pattern_matcher!(ascii, ascii_M, |char| char.is_ascii(), "an ascii character");
define_pattern_matcher!(
	ascii_upper,
	ascii_upper_M,
	|char| char.is_ascii_uppercase(),
	"an ascii uppercase character"
);
define_pattern_matcher!(
	ascii_lower,
	ascii_lower_M,
	|char| char.is_ascii_lowercase(),
	"an ascii lowercase character"
);
define_pattern_matcher!(
	ascii_alpha,
	ascii_alpha_M,
	|char| char.is_ascii_alphabetic(),
	"an ascii alphabetic character"
);
define_pattern_matcher!(
	ascii_alphanum,
	ascii_alphanum_M,
	|char| char.is_ascii_alphanumeric(),
	"an ascii alphanumeric character"
);
define_pattern_matcher!(
	ascii_ws,
	ascii_ws_M,
	|char| char.is_ascii_whitespace(),
	"an ascii whitespace character"
);
define_pattern_matcher!(
	ascii_control,
	ascii_control_M,
	|char| char.is_ascii_control(),
	"an ascii control character"
);
define_pattern_matcher!(
	ascii_printable,
	ascii_printable_M,
	|char| char.is_ascii_graphic(),
	"an ascii printable character"
);
define_pattern_matcher!(
	ascii_punct,
	ascii_punct_M,
	|char| char.is_ascii_punctuation(),
	"an ascii punctuation character"
);
define_pattern_matcher!(dec, dec_M, |char| matches!(char, '0'..='9'), "a decimal digit");
define_pattern_matcher!(
	hex,
	hex_M,
	|char| matches!(char, '0'..='9' | 'a'..='f' | 'A'..='F'),
	"a hexadecimal digit"
);
define_pattern_matcher!(
	hex_lower,
	hex_lower_M,
	|char| matches!(char, '0'..='9' | 'a'..='f'),
	"a lower hexadecimal digit"
);
define_pattern_matcher!(
	hex_upper,
	hex_upper_M,
	|char| matches!(char, '0'..='9' | 'A'..='F'),
	"an upper hexadecimal digit"
);
define_pattern_matcher!(bin, bin_M, |char| matches!(char, '0'..='1'), "a binary digit");
define_pattern_matcher!(
	octal,
	octal_M,
	|char| matches!(char, '0'..='7'),
	"an octal digit"
);

pub fn digit(radix: u8) -> Digit {
	assert!(radix >= 2 && radix <= 36);
	Digit(radix)
}
pub struct Digit(u8);
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
