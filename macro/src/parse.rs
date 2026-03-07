use std::cell::RefCell;

use proc_macro2::{Span, TokenStream};
use syn::{
	Block, Error, Ident, ItemUse, Lit, LitInt, Path, Token, Type, Visibility, bracketed,
	parenthesized,
	parse::{ParseBuffer, discouraged::Speculative},
	punctuated::Punctuated,
	spanned::Spanned,
	token::{Brace, Bracket, Paren},
};

use crate::gen_types::CaptureInfo;

#[derive(Debug, Clone, PartialEq, Copy)]
/// repetition specifiers
///
/// **grammer**: `'?' | '*' | '+' | '[' (exact = nb) ']' | '[' (min? = nb) ".." (max? = nb) ']'`
pub struct Repetition(pub u32, pub u32);
impl Repetition {
	/// no repetition
	pub const ONCE: Self = Self(1, 1);
	/// optional: `?`
	pub const OPTIONAL: Self = Self(0, 1);
	/// more than 0: `*`
	pub const MANY_OPT: Self = Self(0, u32::MAX);
	/// more than 1: `+`
	pub const PLUS1: Self = Self(1, u32::MAX);
}

#[derive(Debug, Clone)]
/// a single mathcer
pub enum Atom {
	/// literals: `1 | "a"`
	Literal(Lit),
	/// path to mathchers: "example::matcher"
	Path(Box<Path>),
	/// match any iterm: **grammer**: `_`
	Any,
	/// enclosed expression: `(exp1 exp2)`,
	///
	/// **grammer**: '(' expr ')'
	Group(Box<Expr>),
	/// call a compound matcher by a set of matchers: `list<dg, ','>`
	///
	/// **grammer**: `path '<' (args = list<expr, ','>) '>'`
	Call { path: Box<Path>, args: Box<[Expr]> },
	/// rust block that resolve to a matcher `{ if expr { matcher1 } else { matcher2 } }`
	Block(Box<Block>),
}

#[derive(Debug, Clone)]
/// the grammer unit
pub enum Expr {
	/// atom with modifiers: `!'a'[3]`
	///
	/// **grammer**: `(not? = '!') (near? = '~') atom rep?`
	Unit { not: bool, near: bool, repetition: Repetition, atom: Atom },
	/// range matcher: `'a'..'z'`
	///
	/// **grammer**: `atom ".." atom`
	Range(Box<Atom>, Atom),
	/// capture the matched section:  `(ident: Ident = 'a'..'z' | 'A'..'Z' | '0'..'9' | '_')`
	///
	/// `'(' ident rep? (':' (ty = path))? = expr ("=>" (conv = block))? ')'`
	Capture {
		ident: Ident,
		rep: Repetition,
		ty: Option<Box<Type>>,
		/// a block that transform the capture
		conv: Option<Box<Block>>,
		type_info: Box<RefCell<CaptureInfo>>,
		expr: Box<Expr>,
	},
	/// sequence of expressions: `'a' 'b' 'c'`
	///
	/// **grammer**:`expr+`
	Seq(Vec<Expr>),
	/// match any of expressions: `'a' | 'b' | 'c'`
	///
	/// **grammer**: `list<expr, '|'>`
	Or(Vec<Expr>),
	/// match all of the expressions: `('a'..'z')[3] & !'a'[3]`
	///
	/// **grammer**: `list<expr, '&'>`
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
		Ok(Repetition::OPTIONAL)
	} else if try_parse!(buf, Token![*]) {
		Ok(Repetition::MANY_OPT)
	} else if try_parse!(buf, Token![+]) {
		Ok(Repetition::PLUS1)
	} else if buf.peek(Bracket) {
		let range;
		bracketed!(range in buf);

		let min = if range.peek(LitInt) { range.parse::<LitInt>()?.base10_parse()? } else { 0 };
		// [exact] variant
		if range.is_empty() && min != 0 {
			return Ok(Repetition(min, min));
		}

		// [min..max] variant
		range.parse::<Token![..]>()?;
		let max =
			if range.peek(LitInt) { range.parse::<LitInt>()?.base10_parse()? } else { u32::MAX };

		if !range.is_empty() {
			return Err(range.error("unexpected token"));
		}
		Ok(Repetition(min, max))
	} else {
		Ok(Repetition::ONCE)
	}
}

fn parse_unit(buf: &ParseBuffer) -> syn::Result<Expr> {
	let start_span = buf.span();
	let not = try_parse!(buf, Token![!]);
	let near = try_parse!(buf, Token![~]);
	let flag_span = (not | near).then_some(start_span);

	let keyword_path_start =
		buf.peek(Token![super]) | buf.peek(Token![self]) | buf.peek(Token![crate]);
	let atom = if buf.peek(Ident) | keyword_path_start {
		let path = buf.call(Path::parse_mod_style)?;
		// call atom
		if buf.peek(Token![<]) {
			buf.parse::<Token![<]>()?;

			let args = Punctuated::<_, Token![,]>::parse_separated_nonempty_with(buf, |buf| {
				parse_expr(buf)
			})?;
			let args = args.into_iter().collect();

			buf.parse::<Token![>]>()?;
			Atom::Call { path: Box::new(path), args }
		} else {
			Atom::Path(Box::new(path))
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
	// test capture start: '(' ident rep? ('=' | ':' & !':')
	// group atom can be (mod::matcher)
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

	Ok(Some(Expr::Capture { ident, rep, ty, conv, type_info: Box::default(), expr }))
}

/// can be used in range
fn is_simple_unit(unit: &Expr) -> bool {
	let Expr::Unit { not, near, repetition, atom, .. } = unit else {
		return false;
	};
	if not | near || *repetition != Repetition::ONCE {
		return false;
	};
	match atom {
		Atom::Block(_) | Atom::Literal(_) | Atom::Path(_) => true,
		Atom::Group(expr) => is_simple_unit(expr),
		_ => false,
	}
}

/// parse range if can, else a single unit
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

// parses expr at different levels: or -> seq -> and
fn at_expr_end(buf: &ParseBuffer) -> bool {
	// the regular one like , and ;
	// '>': at end of call atom
	// '=': at end of capture before conv block
	let is_comm = buf.peek(Token![,]);
	is_comm || buf.peek(Token![;]) || buf.peek(Token![>]) || buf.peek(Token![=]) || buf.is_empty()
}
fn parse_expr_and(buf: &ParseBuffer) -> syn::Result<Expr> {
	let expr = parse_expr_range(buf)?;
	if buf.peek(Token![|]) || at_expr_end(buf) {
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
	if buf.peek(Token![|]) || at_expr_end(buf) {
		return Ok(expr);
	}
	let mut exprs = vec![expr];
	while !(buf.peek(Token![|]) || at_expr_end(buf)) {
		exprs.push(parse_expr_and(buf)?);
	}
	Ok(Expr::Seq(exprs))
}

pub fn parse_expr(buf: &ParseBuffer) -> syn::Result<Expr> {
	let expr = parse_expr_seq(buf)?;
	if buf.peek(Token![&]) || at_expr_end(buf) {
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

/// a term in gramex macro: `let arr<M> = '[' list<M, ','> ']';`
///
/// **grammer**: `"let" ident ('<' (args = list<ident, ','>) '>')? (':' (ty = path))? = expr ("=>" (conv = block))?`
#[derive(Debug, Clone)]
pub struct Term {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub resolved_type: TokenStream,
	pub expr: Expr,
}
/// the gramex macro
///
/// **grammer**: `
///   (visibility (mod_name = ident))? 'for' (matched_type = ty) ';'
///   (use_decls* = use_decl) (terms* = term)
/// `
#[derive(Debug, Clone)]
pub struct GramexMacro {
	pub mod_vis: Visibility,
	pub mod_name: Option<Ident>,
	pub use_decls: Vec<ItemUse>,
	pub matched_type: Type,
	pub terms: Vec<Term>,
}
pub fn parse_gramex_macro(buf: &ParseBuffer) -> syn::Result<GramexMacro> {
	let mod_vis = buf.parse::<Visibility>()?;
	let mod_name = if try_parse!(buf, Token![mod]) { Some(buf.parse()?) } else { None };
	if mod_vis != Visibility::Inherited && mod_name.is_none() {
		return Err(Error::new(mod_vis.span(), "visibility without module specifier"));
	}
	buf.parse::<Token![for]>()?;
	let matched_type = buf.parse()?;
	buf.parse::<Token![;]>()?;

	let mut use_decls = Vec::new();
	while buf.peek(Token![use]) {
		use_decls.push(buf.parse::<ItemUse>()?);
	}
	if !use_decls.is_empty() && mod_name.is_none() {
		return Err(Error::new(use_decls[0].span(), "use decleration without module specifier"));
	}

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
		buf.parse::<Token![;]>()?;

		// wrap the root expr with a root capture
		let cap_ident = Ident::new("root", name.span());
		#[rustfmt::skip]
		let expr = Expr::Capture {
			ident: cap_ident, rep: Repetition::ONCE, ty, conv, type_info: Box::default(), expr,
		};
		terms.push(Term { name, args, expr, resolved_type: TokenStream::new() });
	}

	Ok(GramexMacro { mod_vis, mod_name, use_decls, matched_type, terms })
}

pub struct MatcherExpr {
	pub value: syn::Expr,
	pub expr: Expr,
	pub matched_type: Type,
}
pub fn parse_matcher_expr(buf: &ParseBuffer) -> syn::Result<MatcherExpr> {
	let value = buf.parse::<syn::Expr>()?;
	buf.parse::<Token![:]>()?;
	let matched_type = buf.parse()?;
	buf.parse::<Token![,]>()?;

	let expr = Box::new(parse_expr(buf)?);
	// wrap the root expr with a root capture
	let cap_ident = Ident::new("root", Span::call_site());
	#[rustfmt::skip]
	let expr = Expr::Capture {
		ident: cap_ident, rep: Repetition::ONCE, ty: None, conv: None, 
		type_info: Box::default(), expr,
	};

	Ok(MatcherExpr { value, expr, matched_type })
}
