use alloc_crate::{borrow::Cow, string::String};
use core::{
	fmt::{Display, Write},
	ops::{Range, RangeInclusive},
};
use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	core::{define_ref_matcher, define_token_matcher, match_token},
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
	($name:ident, |$char:ident| $logic:expr, $kind:literal) => {
		#[allow(nonstandard_style)]
		pub struct $name;
		define_token_matcher!($name, str, |_matcher, $char| (
			$logic.then_some($char.len_utf8()),
			Expected::A(LeanString::from_static_str($kind))
		));
	};
}

define_pattern_matcher!(upper, |char| char.is_uppercase(), "an uppercase character");
define_pattern_matcher!(lower, |char| char.is_lowercase(), "a lowercase character");
define_pattern_matcher!(alpha, |char| char.is_alphabetic(), "an alphabetic character");
define_pattern_matcher!(num, |char| char.is_numeric(), "a numeric character");
define_pattern_matcher!(
	alphanum,
	|char| char.is_alphanumeric(),
	"an alphanumeric character"
);
define_pattern_matcher!(ws, |char| char.is_whitespace(), "a whitespace character");
define_pattern_matcher!(control, |char| char.is_control(), "a control character");
define_pattern_matcher!(ascii, |char| char.is_ascii(), "an ascii character");
define_pattern_matcher!(
	ascii_upper,
	|char| char.is_ascii_uppercase(),
	"an ascii uppercase character"
);
define_pattern_matcher!(
	ascii_lower,
	|char| char.is_ascii_lowercase(),
	"an ascii lowercase character"
);
define_pattern_matcher!(
	ascii_alpha,
	|char| char.is_ascii_alphabetic(),
	"an ascii alphabetic character"
);
define_pattern_matcher!(
	ascii_alphanum,
	|char| char.is_ascii_alphanumeric(),
	"an ascii alphanumeric character"
);
define_pattern_matcher!(
	ascii_ws,
	|char| char.is_ascii_whitespace(),
	"an ascii whitespace character"
);
define_pattern_matcher!(
	ascii_control,
	|char| char.is_ascii_control(),
	"an ascii control character"
);
define_pattern_matcher!(
	ascii_printable,
	|char| char.is_ascii_graphic(),
	"an ascii printable character"
);
define_pattern_matcher!(
	ascii_punct,
	|char| char.is_ascii_punctuation(),
	"an ascii punctuation character"
);
define_pattern_matcher!(dec, |char| matches!(char, '0'..='9'), "a decimal digit");
define_pattern_matcher!(
	hex,
	|char| matches!(char, '0'..='9' | 'a'..='f' | 'A'..='F'),
	"a hexadecimal digit"
);
define_pattern_matcher!(
	hex_lower,
	|char| matches!(char, '0'..='9' | 'a'..='f'),
	"a lower hexadecimal digit"
);
define_pattern_matcher!(
	hex_upper,
	|char| matches!(char, '0'..='9' | 'A'..='F'),
	"an upper hexadecimal digit"
);
define_pattern_matcher!(bin, |char| matches!(char, '0'..='1'), "a binary digit");
define_pattern_matcher!(octal, |char| matches!(char, '0'..='7'), "an octal digit");

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
