use std::cell::Cell;

use proc_macro2::{Delimiter, Span, TokenStream};
use syn::{
	Block, Error, Ident, Lit, LitInt, Token, Type, bracketed, parenthesized,
	parse::{ParseBuffer, discouraged::Speculative},
	punctuated::Punctuated,
	spanned::Spanned,
	token::{Brace, Paren},
};

type Path = Punctuated<Ident, Token![::]>;

#[derive(Debug, Clone, PartialEq, Copy)]
/// repetition specifiers
///
/// `'[' (min? = nb) ".." (max? = nb) ']'`
pub struct Repetition(pub u32, pub u32);
impl Repetition {
	/// normal
	pub const Once: Self = Self(1, 1);
	/// `?`
	pub const Optional: Self = Self(0, 1);
	/// `*`
	pub const ManyOpt: Self = Self(0, u32::MAX);
	/// `+`
	pub const Plus1: Self = Self(1, u32::MAX);
}

#[derive(Debug, Clone, PartialEq)]
/// a signle mathcer
pub enum Atom {
	/// `1 | "a"`, **grammer**: rust ident
	Literal(Lit),
	/// `example::dg`, **grammer**: rust path
	Term(Path),
	/// `_`
	Any,
	/// `(exp1 exp2)`, **grammer**: '(' expr ')'
	Group(Box<Expr>),
	/// `list<dg, ','>`
	///
	/// **grammer**: `path '<' (args = list<expr, ','>) '>'`
	Call { path: Box<Path>, args: Box<[Expr]> },
	/// `{ |inp| custom_matcher(inp) }`, **grammer**: rust block
	Block(Box<Block>),
}

#[derive(Debug, Clone, PartialEq)]
/// the grammer unit
pub enum Expr {
	/// atom with modifiers
	///
	/// **grammer**: `(flags = '!'? '~'?) atom rep?`
	Unit { not: bool, near: bool, repetition: Repetition, atom: Atom },
	/// `'a'..'z'`
	///
	/// `atom ".." atom`
	Range(Box<Atom>, Atom),
	/// `(ident: Ident = 'a'..'z' | 'A'..'Z' | '0'..'9' | '_')`
	///
	/// `'(' ident rep? (':' (ty = path))? = expr ("=>" (conv = block))? ')'`
	Capture {
		ident: Ident,
		rep: Repetition,
		ty: Option<Box<Type>>,
		conv: Option<Box<Block>>,
		typeid: Cell<u32>,
		expr: Box<Expr>,
	},
	/// sequence of expressions
	///
	/// `exp1 exp2 exp3`
	Seq(Vec<Expr>),
	/// match any of expressions
	///
	/// `exp1 | exp2 | exp3`
	Or(Vec<Expr>),
	/// match all of the expressions
	///
	/// `exp1 & exp2 & exp3`
	And(Vec<Expr>),
}

macro_rules! try_parse {
	($buf:ident, $($token:tt)+) => {
		if $buf.peek($($token)+) {
			$buf.parse::<$($token)+>().is_ok()
		} else {
			false
		}
	};
}

fn parse_repetition(buf: &ParseBuffer) -> syn::Result<Repetition> {
	if try_parse!(buf, Token![?]) {
		Ok(Repetition::Optional)
	} else if try_parse!(buf, Token![*]) {
		Ok(Repetition::ManyOpt)
	} else if try_parse!(buf, Token![+]) {
		Ok(Repetition::Plus1)
	} else if buf.cursor().group(Delimiter::Bracket).is_some() {
		let range;
		bracketed!(range in buf);

		let min = if range.peek(LitInt) { range.parse::<LitInt>()?.base10_parse()? } else { 0 };
		if range.is_empty() {
			return Ok(Repetition(min, min));
		}
		range.parse::<Token![..]>()?;
		let max =
			if range.peek(LitInt) { range.parse::<LitInt>()?.base10_parse()? } else { u32::MAX };

		if !range.is_empty() {
			return Err(range.error("unexpected token"));
		}
		Ok(Repetition(min, max))
	} else {
		Ok(Repetition::Once)
	}
}

fn parse_unit(buf: &ParseBuffer) -> syn::Result<Expr> {
	let start_span = buf.span();
	let not = try_parse!(buf, Token![!]);
	let near_span = buf.span();
	let near = try_parse!(buf, Token![~]);
	let mut flag_span = None;
	if near | not {
		flag_span = Span::join(&start_span, if near { near_span } else { start_span })
	};

	let atom = if buf.peek(Ident) {
		let path = buf.call(Path::parse_separated_nonempty)?;
		if buf.peek(Token![<]) {
			buf.parse::<Token![<]>()?;

			let args = Punctuated::<_, Token![,]>::parse_separated_nonempty_with(buf, |buf| {
				parse_expr(buf)
			})?;
			let args = args.into_iter().collect();

			buf.parse::<Token![>]>()?;
			Atom::Call { path: Box::new(path), args }
		} else {
			Atom::Term(path)
		}
	} else if buf.peek(Lit) {
		Atom::Literal(buf.parse()?)
	} else if try_parse!(buf, Token![_]) {
		Atom::Any
	} else if buf.peek(Brace) {
		Atom::Block(Box::new(buf.parse()?))
	} else if buf.peek(Paren) {
		let expr;
		parenthesized!(expr in buf);
		if let Some(expr) = try_parse_capture(&expr, flag_span)? {
			return Ok(expr);
		}
		Atom::Group(Box::new(parse_expr(&expr)?))
	} else {
		return Err(buf.error("expected atom"));
	};

	let repetition = parse_repetition(buf)?;

	Ok(Expr::Unit { not, near, repetition, atom })
}

fn try_parse_capture(buf: &ParseBuffer, flag_span: Option<Span>) -> syn::Result<Option<Expr>> {
	let fork = buf.fork();
	let Ok(ident) = fork.parse::<Ident>() else {
		return Ok(None);
	};
	let rep = parse_repetition(&fork)?;

	if !(fork.peek(Token![=]) || (fork.peek(Token![:]) && !fork.peek2(Token![:]))) {
		return Ok(None);
	}
	buf.advance_to(&fork);
	if let Some(span) = flag_span {
		return Err(Error::new(span, "capture can not have flags"));
	}

	let mut ty = None;
	if try_parse!(buf, Token![:]) {
		ty = Some(Box::new(buf.parse::<Type>()?));
	}

	buf.parse::<Token![=]>()?;
	let expr = Box::new(parse_expr(buf)?);

	let mut conv = None;
	if try_parse!(buf, Token![=>]) {
		conv = Some(Box::new(buf.parse::<Block>()?));
	}

	Ok(Some(Expr::Capture { ident, rep, ty, conv, typeid: Cell::new(0), expr }))
}

fn is_simple_unit(unit: &Expr) -> bool {
	let Expr::Unit { not, near, repetition, atom, .. } = unit else {
		return false;
	};
	if not | near || *repetition != Repetition::Once {
		return false;
	};
	match atom {
		Atom::Block(_) | Atom::Literal(_) | Atom::Term(_) => true,
		Atom::Group(expr) => is_simple_unit(expr),
		_ => false,
	}
}

fn parse_expr_range(buf: &ParseBuffer) -> syn::Result<Expr> {
	let lh = parse_unit(buf)?;

	if buf.peek(Token![..]) {
		let op_span = buf.parse::<Token![..]>()?.span();
		if !is_simple_unit(&lh) {
			return Err(Error::new(op_span, "left side of range must be a simple unit"));
		}
		let rh = parse_unit(buf)?;
		if !is_simple_unit(&rh) {
			return Err(Error::new(op_span, "right side of range must be a simple unit"));
		}

		let (Expr::Unit { atom: lh, .. }, Expr::Unit { atom: rh, .. }) = (lh, rh) else {
			unreachable!()
		};
		return Ok(Expr::Range(Box::new(lh), rh));
	}

	Ok(lh)
}

fn expr_end(buf: &ParseBuffer) -> bool {
	let is_comm = buf.peek(Token![,]);
	is_comm || buf.peek(Token![;]) || buf.peek(Token![>]) || buf.peek(Token![=]) || buf.is_empty()
}
fn parse_expr_and(buf: &ParseBuffer) -> syn::Result<Expr> {
	let expr = parse_expr_range(buf)?;
	if buf.peek(Token![|]) || expr_end(buf) {
		return Ok(expr);
	}
	if !buf.peek(Token![&]) {
		return Ok(expr);
	}
	let mut exprs = vec![expr];
	while try_parse!(buf, Token![&]) {
		exprs.push(parse_expr_range(buf)?);
	}
	Ok(Expr::And(exprs))
}
fn parse_expr_seq(buf: &ParseBuffer) -> syn::Result<Expr> {
	let expr = parse_expr_and(buf)?;
	if buf.peek(Token![|]) || expr_end(buf) {
		return Ok(expr);
	}
	let mut exprs = vec![expr];
	while !(buf.peek(Token![|]) || expr_end(buf)) {
		exprs.push(parse_expr_and(buf)?);
	}
	Ok(Expr::Seq(exprs))
}

pub fn parse_expr(buf: &ParseBuffer) -> syn::Result<Expr> {
	let expr = parse_expr_seq(buf)?;
	if buf.peek(Token![&]) || expr_end(buf) {
		return Ok(expr);
	}
	if !buf.peek(Token![|]) {
		return Ok(expr);
	}
	let mut exprs = vec![expr];
	while try_parse!(buf, Token![|]) {
		exprs.push(parse_expr_seq(buf)?);
	}
	Ok(Expr::Or(exprs))
}

/// **grammer**: `"let" ident ('<' (args = list<ident, ','>) '>')? (':' (ty = path))? = expr ("=>" (conv = block))?`
#[derive(Debug, Clone)]
pub struct Term {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub resolved: TokenStream,
	pub expr: Expr,
}
#[derive(Debug, Clone)]
pub struct GramexMacro {
	pub mod_name: Option<Ident>,
	pub matched_type: Type,
	pub terms: Vec<Term>,
}
pub fn parse_gramex_macro(buf: &ParseBuffer) -> syn::Result<GramexMacro> {
	try_parse!(buf, Token![pub]);
	let mod_name = if try_parse!(buf, Token![mod]) { Some(buf.parse()?) } else { None };
	buf.parse::<Token![for]>()?;
	let matched_type = buf.parse()?;

	let mut terms = Vec::new();
	while buf.peek(Token![let]) {
		buf.parse::<Token![let]>()?;
		let name = buf.parse::<Ident>()?;

		let mut args = Vec::new();
		if buf.peek(Token![<]) {
			buf.parse::<Token![<]>()?;
			let args_punc = buf.call(Punctuated::<Ident, Token![,]>::parse_separated_nonempty)?;
			args = args_punc.into_iter().collect();
			buf.parse::<Token![>]>()?;
		}

		let mut ty = None;
		if try_parse!(buf, Token![:]) {
			ty = Some(Box::new(buf.parse::<Type>()?));
		}

		buf.parse::<Token![=]>()?;
		let expr = Box::new(parse_expr(buf)?);

		let mut conv = None;
		if try_parse!(buf, Token![=>]) {
			conv = Some(Box::new(buf.parse::<Block>()?));
		}

		#[rustfmt::skip]
		let expr = Expr::Capture {
			ident: name.clone(), rep: Repetition::Once, ty, conv, typeid: Cell::new(0), expr,
		};
		buf.parse::<Token![;]>()?;
		terms.push(Term { name, args, expr, resolved: TokenStream::new() });
	}

	Ok(GramexMacro { mod_name, matched_type, terms })
}
