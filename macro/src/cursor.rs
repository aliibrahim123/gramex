//! defines the [`Cursor`] struct.

use std::{fmt::Display, str::FromStr};

use chunked_quote::chunk_spanned;
use proc_macro2::{
	Delimiter, Group, Ident, Literal, Spacing, Span, TokenStream, TokenTree,
};
use quote::ToTokens;

macro_rules! ident {
	($format:literal, span = $span:expr, $($t:tt)*) => {
		Ident::new(&format!($format, $($t)*), $span)
	};
	($format:literal $($t:tt)*) => {
		Ident::new(&format!($format $($t)*), Span::call_site())
	};
}
pub(crate) use ident;

/// reducing boilerplate of parsing [`TokenStream`].
#[derive(Debug)]
pub struct Cursor<'a> {
	pub tokens: Box<[TokenTree]>,
	pub ind: usize,
	pub end_span: Span,
	pub errors: &'a mut Vec<Error>,
}
impl Cursor<'_> {
	pub fn new<'a>(
		stream: TokenStream, end_span: Span, errors: &'a mut Vec<Error>,
	) -> Cursor<'a> {
		Cursor { tokens: stream.into_iter().collect(), ind: 0, end_span, errors }
	}
	pub fn peek_next(&self, n: usize) -> Option<&TokenTree> {
		self.tokens.get(self.ind + n)
	}
	/// returns the current [`TokenTree`]
	pub fn peek(&self) -> Option<&TokenTree> {
		self.tokens.get(self.ind)
	}
	/// returns the current [`TokenTree`] span
	pub fn cur_span(&self) -> Span {
		self.peek().map_or(self.end_span, TokenTree::span)
	}
	/// skips the current [`TokenTree`]
	pub fn skip(&mut self) {
		self.ind += 1;
	}
	/// returns the previous [`TokenTree`]
	pub fn prev(&self) -> &TokenTree {
		&self.tokens[self.ind - 1]
	}
	/// returns `true` if the cursor is at the end
	pub fn is_end(&self) -> bool {
		self.ind >= self.tokens.len()
	}
	/// sets the current index
	pub fn rewind(&mut self, ind: usize) {
		self.ind = ind
	}
	pub fn expected(&mut self, expected: impl Display) {
		err!(self, "expected {expected}");
	}
	/// eat a [`Punct`] of a specific character
	pub fn punct(&mut self, char: char) -> Option<Span> {
		if self.try_punct(char) {
			Some(self.prev().span())
		} else {
			err!(self, "expected `{char}`");
			None
		}
	}
	/// try eat a [`Punct`] of a specific character
	pub fn try_punct(&mut self, char: char) -> bool {
		let Some(TokenTree::Punct(punct)) = self.peek() else { return false };
		if punct.as_char() != char {
			return false;
		}
		self.skip();
		true
	}
	pub fn test_punct_alone(&mut self, char: char) -> bool {
		let Some(TokenTree::Punct(punct)) = self.peek() else { return false };
		punct.as_char() == char && punct.spacing() == Spacing::Alone
	}
	pub fn test_punct(&self, char: char) -> bool {
		let Some(TokenTree::Punct(punct)) = self.peek() else { return false };
		punct.as_char() == char
	}
	/// eat multiple [`Punct`]s of specific characters
	pub fn multi_punct<const N: usize>(
		&mut self, chars: [char; N],
	) -> Option<TokenStream> {
		if self.try_multi_punct(chars) {
			Some(TokenStream::from_iter(
				self.tokens[self.ind - N..self.ind].iter().cloned(),
			))
		} else {
			let chars = chars.iter().collect::<String>();
			err!(self, "expected `{chars}");
			None
		}
	}
	pub fn try_multi_punct<const N: usize>(&mut self, chars: [char; N]) -> bool {
		self.test_multi_punct(chars).then(|| self.ind += N).is_some()
	}
	/// try eat multiple [`Punct`]s of specific characters
	pub fn test_multi_punct<const N: usize>(&self, chars: [char; N]) -> bool {
		// head
		for i in 0..N - 1 {
			if !matches!(self.peek_next(i), Some(TokenTree::Punct(punct))
				if punct.as_char() == chars[i] && punct.spacing() == Spacing::Joint
			) {
				return false;
			}
		}
		matches!(self.peek_next(N - 1), Some(TokenTree::Punct(punct))
			if punct.as_char() == chars[N - 1]
		)
	}
	/// eat an [`Ident`]
	pub fn ident(&mut self) -> Option<Ident> {
		if let Some(ident) = self.try_ident() {
			Some(ident)
		} else {
			err!(self, "expected an identifier");
			None
		}
	}
	/// try eat an [`Ident`]
	pub fn try_ident(&mut self) -> Option<Ident> {
		let Some(TokenTree::Ident(ident)) = self.peek() else { return None };
		let ident = ident.clone();
		self.skip();
		Some(ident)
	}
	/// eat a specific [`Ident`]
	pub fn kw(&mut self, kw: &str) -> Option<Ident> {
		if self.try_kw(kw) {
			let TokenTree::Ident(ident) = self.prev() else { unreachable!() };
			Some(ident.clone())
		} else {
			err!(self, "expected `{kw}`");
			None
		}
	}
	/// try eat a specific [`Ident`]
	pub fn try_kw(&mut self, kw: &str) -> bool {
		let Some(TokenTree::Ident(ident)) = self.peek() else { return false };
		if ident != kw {
			return false;
		}
		self.skip();
		true
	}
	pub fn test_kw(&self, kw: &str) -> bool {
		let Some(TokenTree::Ident(ident)) = self.peek() else { return false };
		ident == kw
	}
	/// eat a [`Literal`]
	pub fn literal(&mut self) -> Option<Literal> {
		if let Some(lit) = self.try_literal() {
			Some(lit)
		} else {
			err!(self, "expected a literal");
			None
		}
	}
	/// try eat a [`Literal`]
	pub fn try_literal(&mut self) -> Option<Literal> {
		let Some(TokenTree::Literal(lit)) = self.peek() else { return None };
		let lit = lit.clone();
		self.skip();
		Some(lit)
	}
	pub fn nb<T: FromStr>(&mut self) -> Option<T> {
		if let Some(nb) = self.try_nb() {
			Some(nb)
		} else {
			err!(self, "expected a number");
			None
		}
	}
	pub fn try_nb<T: FromStr>(&mut self) -> Option<T> {
		let Some(TokenTree::Literal(lit)) = self.peek() else { return None };
		let Ok(nb) = lit.to_string().parse::<T>() else { return None };
		self.skip();
		Some(nb)
	}
	/// eat a [`Group`] of a specific [`Delimiter`]
	pub fn group(&mut self, delim: Delimiter) -> Option<Group> {
		if let Some(group) = self.try_group(delim) {
			Some(group)
		} else {
			let bracket = match delim {
				Delimiter::Parenthesis => "(",
				Delimiter::Brace => "{",
				Delimiter::Bracket => "[",
				Delimiter::None => panic!(),
			};
			err!(self, "expected `{bracket}`");
			None
		}
	}
	/// try eat a [`Group`] of a specific [`Delimiter`]
	pub fn try_group(&mut self, delim: Delimiter) -> Option<Group> {
		let Some(TokenTree::Group(group)) = self.peek() else { return None };
		if group.delimiter() != delim {
			return None;
		}
		let group = group.clone();
		self.skip();
		Some(group)
	}
	/// creates a [`Cursor`] for the stream of a [`Group`] of a specific [`Delimiter`]
	pub fn enter_group(&mut self, delim: Delimiter) -> Option<Cursor<'_>> {
		let group = self.group(delim)?;
		Some(Cursor::new(group.stream(), group.span_close(), self.errors))
	}
	/// try creates a [`Cursor`] for the stream of a [`Group`] of a specific [`Delimiter`]
	pub fn try_enter_group(&mut self, delim: Delimiter) -> Option<Cursor> {
		let group = self.try_group(delim)?;
		Some(Cursor::new(group.stream(), group.span_close(), self.errors))
	}

	pub fn eat_until(
		&mut self, expected: impl Display, pred: impl Fn(&mut Self) -> bool,
	) -> Option<TokenStream> {
		let tokens = self.try_eat_until(pred);
		if tokens.is_empty() {
			self.expected(expected);
			return None;
		}
		Some(tokens)
	}
	pub fn try_eat_until(&mut self, pred: impl Fn(&mut Self) -> bool) -> TokenStream {
		let start = self.ind;
		while !self.is_end() && !pred(self) {
			self.skip();
		}
		TokenStream::from_iter(self.tokens[start..self.ind].iter().cloned())
	}
}

/// [`TokenTree`] parsing error
#[derive(Debug, Clone)]
pub struct Error {
	msg: String,
	span: Span,
}
impl Error {
	pub fn new(msg: String, span: Span) -> Self {
		Self { msg, span }
	}
}

impl ToTokens for Error {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let Self { msg, span } = self;
		chunk_spanned!(tokens, *span, ::core::compile_error!(#msg););
	}
}

/// simplifies [`Error`] creation
macro_rules! err {
	($cur:ident, $msg:literal $(, )?) => {{
		$cur.errors.push(Error::new(format!($msg), $cur.cur_span()));
	}};
	($cur:ident, $msg:literal, $span:expr) => {{
		$cur.errors.push(Error::new(format!($msg), $span));
	}};
	($cur:ident, $msg:expr, $span:expr) => {{
		$cur.errors.push(Error::new($msg.to_string(), $span));
	}};
}
pub(crate) use err;
