use chunked_quote::quote;
use proc_macro2::{
	Delimiter::{self, Brace, Bracket, Parenthesis},
	Group, Ident, Literal, Spacing, Span, TokenStream, TokenTree,
};

use crate::{
	analyze::CapInfo,
	cursor::{Cursor, Error, err},
};

/// repetition specifiers
///
/// **grammer**: `'?' | '*' | '+' | '[' exact:nb ']' | '[' min?:nb ".." max?:nb ']'`
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Rep(pub u32, pub u32);
impl Rep {
	/// no repetition
	pub const ONCE: Self = Self(1, 1);
	/// optional: `?`
	pub const OPTIONAL: Self = Self(0, 1);
	/// more than 0: `*`
	pub const MANY_OPT: Self = Self(0, u32::MAX);
	/// more than 1: `+`
	pub const PLUS1: Self = Self(1, u32::MAX);

	fn exact(n: u32) -> Self {
		Self(n, n)
	}
	fn is_exact(self) -> bool {
		self.0 == self.1
	}
}

#[derive(Debug, Clone)]
/// a single mathcer
pub enum Atom {
	// literal, blocks and paths that resolve to a matcher
	Matcher(TokenTree),
	/// match any item: `_`
	Any,
	/// enclosed expression,
	Group(Box<Expr>),
	/// call to compound matcher: `path '<' args:list<matcher, ','> '>'`
	Call {
		path: Box<[TokenTree]>,
		args: Box<[Matcher]>,
	},
}

/// capture type specifier
#[derive(Debug, Clone)]
pub enum CapType {
	Inherited,
	/// **grammer**: `':' type`
	Explicit(TokenStream),
	// **grammer**: `':' "struct" ident?`
	Struct(Option<Ident>),
	/// **grammer**: `':' "enum" ident?`
	Enum(Option<Ident>),
}

/// capture of matched section
///
/// **grammer**: `ident rep? ':' atom |  '(' ident rep? (":" -> cap_type) '=' expr ("=>" -> map:expr) ')'`
#[derive(Debug, Clone)]
pub struct Capture {
	pub ident: Ident,
	pub rep: Rep,
	pub ty: CapType,
	// a block that transform the capture
	pub map: Option<TokenStream>,
	pub expr: Expr,
	pub info: Option<CapInfo>,
}

impl Default for Capture {
	fn default() -> Self {
		Self {
			ident: Ident::new("default", Span::call_site()),
			rep: Rep::ONCE,
			ty: CapType::Inherited,
			map: None,
			expr: Expr::Error,
			info: None,
		}
	}
}

#[derive(Debug, Clone)]
/// the grammer unit
pub enum Expr {
	/// atom with modifiers: `not?:'!' near?:'~' atom rep?`
	Unit {
		not: bool,
		near: bool,
		rep: Rep,
		atom: Atom,
	},
	/// range matcher: `atom ".." atom`
	Range(TokenTree, TokenTree),
	Capture(Box<Capture>),
	/// sequence of expressions: `expr+`
	Seq(Vec<Expr>),
	/// match any of expressions: `list<expr, '|'>`
	Or(Vec<Expr>),
	/// match all of the expressions: `list<expr, '&'>`
	And(Vec<Expr>),
	/// matches `expr` if `cond` matches, else match nothing: `expr -> expr`
	Imply {
		cond: Box<Expr>,
		expr: Box<Expr>,
	},
	/// error encountered during parsing
	Error,
}

fn is_expr_end(cur: &Cursor) -> bool {
	let is_end = cur.is_end() || cur.test_punct(',') || cur.test_punct(';');
	is_end || cur.test_punct('>') || cur.test_multi_punct(['=', '>']) | cur.test_kw("let")
}

fn try_parse_path(cur: &mut Cursor) -> Option<Vec<TokenTree>> {
	let start = cur.ind;
	let mut segments = Vec::new();
	if cur.try_multi_punct([':', ':']) {
		segments.extend(cur.tokens[cur.ind - 2..cur.ind].iter().cloned());
	}
	let Some(first_ident) = cur.try_ident() else {
		cur.rewind(start);
		return None;
	};
	segments.push(first_ident.into());
	while cur.try_multi_punct([':', ':']) {
		let Some(ident) = cur.ident() else { break };
		segments.extend(cur.tokens[cur.ind - 3..cur.ind - 1].iter().cloned());
		segments.push(ident.into());
	}
	Some(segments)
}

fn parse_rep_bracket(cur: &mut Cursor) -> Rep {
	let min = cur.try_nb();
	let rep = if cur.try_multi_punct(['.', '.']) {
		let max = cur.try_nb().unwrap_or(u32::MAX);
		Rep(min.unwrap_or(0), max)
	} else {
		min.map(Rep::exact).unwrap_or_else(|| {
			cur.expected("a number");
			Rep::ONCE
		})
	};
	if !cur.is_end() {
		cur.expected("`]`");
	}
	rep
}

fn parse_rep(cur: &mut Cursor) -> Rep {
	if cur.try_punct('?') {
		Rep::OPTIONAL
	} else if cur.try_punct('*') {
		Rep::MANY_OPT
	} else if cur.try_punct('+') {
		Rep::PLUS1
	} else if let Some(mut cur) = cur.try_enter_group(Bracket) {
		parse_rep_bracket(&mut cur)
	} else {
		Rep::ONCE
	}
}

fn parse_atom_path(cur: &mut Cursor, path: Vec<TokenTree>) -> Option<Atom> {
	if cur.try_punct('<') {
		let mut args = Vec::new();
		if !cur.test_punct('>') {
			args.push(parse_matcher(cur, true));
			while cur.try_punct(',') && !cur.test_punct('>') {
				args.push(parse_matcher(cur, true));
			}
		}
		cur.punct('>')?;
		Some(Atom::Call { path: path.into_boxed_slice(), args: args.into_boxed_slice() })
	} else {
		Some(Atom::Matcher(TokenTree::Group(Group::new(
			Delimiter::None,
			TokenStream::from_iter(path),
		))))
	}
}
fn parse_atom(cur: &mut Cursor) -> Option<Atom> {
	if cur.try_kw("_") {
		Some(Atom::Any)
	} else if let Some(lit) = cur.try_literal() {
		Some(Atom::Matcher(lit.into()))
	} else if let Some(block) = cur.try_group(Brace) {
		Some(Atom::Matcher(block.into()))
	} else if let Some(path) = try_parse_path(cur) {
		parse_atom_path(cur, path)
	} else {
		cur.expected("an atom");
		if !is_expr_end(cur) {
			cur.skip();
		}
		None
	}
}

fn parse_capture_type(cur: &mut Cursor) -> CapType {
	if !cur.try_punct(':') {
		CapType::Inherited
	} else if cur.try_kw("struct") {
		CapType::Struct(cur.try_ident())
	} else if cur.try_kw("enum") {
		CapType::Enum(cur.try_ident())
	} else {
		cur.eat_until("a type", |cur| cur.test_punct('='))
			.map(CapType::Explicit)
			.unwrap_or(CapType::Inherited)
	}
}

fn try_inline_capture(cur: &mut Cursor) -> Option<Expr> {
	let start = cur.ind;
	let ident = cur.try_ident()?;
	let rep = parse_rep(cur);

	if !cur.try_punct(':') || cur.test_punct(':') {
		cur.rewind(start);
		return None;
	}

	let expr = if let Some(atom) = parse_atom(cur) {
		Expr::Unit { not: false, near: false, atom, rep: Rep::ONCE }
	} else {
		Expr::Error
	};

	let cap = Capture { ident, rep, expr, ..Default::default() };
	Some(Expr::Capture(Box::new(cap)))
}
fn try_parse_capture(cur: &mut Cursor, flags_span: Option<Span>) -> Option<Expr> {
	let start = cur.ind;
	let Some(ident) = cur.try_ident() else { return None };
	let rep = parse_rep(cur);

	if !cur.test_punct('=') && !(cur.test_punct(':') && !cur.test_multi_punct([':', ':'])) {
		cur.rewind(start);
		return None;
	}
	if let Some(flags_span) = flags_span {
		err!(cur, "capture can not have modifiers", flags_span);
	}

	let ty = parse_capture_type(cur);
	cur.punct('=');

	let expr = parse_expr(cur);
	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until("an expression", |cur| cur.is_end()),
		false => None,
	};
	if !cur.is_end() {
		cur.expected("`)`");
	}

	let cap = Capture { ident, rep, ty, map, expr, info: None };
	Some(Expr::Capture(Box::new(cap)))
}

fn parse_range(cur: &mut Cursor, atom: Atom, not: bool, near: bool, flags_span: Span) -> Expr {
	let Atom::Matcher(left) = atom else {
		err!(cur, "expected a value atom", flags_span);
		return Expr::Unit { not, near, rep: Rep::ONCE, atom };
	};
	if near | not {
		err!(cur, "range can not have modifiers", flags_span);
	}

	let right_span = cur.cur_span();
	let right = match parse_atom(cur) {
		Some(Atom::Matcher(right)) => right,
		Some(atom) => {
			err!(cur, "expected a value atom", right_span);
			return Expr::Seq(vec![
				Expr::Unit { not, near, rep: Rep::ONCE, atom: Atom::Matcher(left) },
				Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom },
			]);
		}
		None => return Expr::Unit { not, near, rep: Rep::ONCE, atom: Atom::Matcher(left) },
	};

	Expr::Range(left, right)
}

fn parse_expr_primary(cur: &mut Cursor) -> Expr {
	if let Some(expr) = try_inline_capture(cur) {
		return expr;
	}
	let flags_span = cur.cur_span();
	let not = cur.try_punct('!');
	let near = cur.try_punct('~');
	let atom = if let Some(mut cur) = cur.try_enter_group(Parenthesis) {
		if let Some(expr) = try_parse_capture(&mut cur, (not | near).then_some(flags_span)) {
			return expr;
		}
		let expr = parse_expr(&mut cur);
		if !cur.is_end() {
			cur.expected("`)`");
		}
		Atom::Group(Box::new(expr))
	} else if let Some(atom) = parse_atom(cur) {
		atom
	} else {
		return Expr::Error;
	};
	if cur.try_multi_punct(['.', '.']) {
		parse_range(cur, atom, not, near, flags_span)
	} else {
		let rep = parse_rep(cur);
		Expr::Unit { not, near, rep, atom }
	}
}

fn parse_chain(
	cur: &mut Cursor, parse_item: impl Fn(&mut Cursor) -> Expr, sep: impl Fn(&mut Cursor) -> bool,
	item: impl Fn(Vec<Expr>) -> Expr,
) -> Expr {
	let expr = parse_item(cur);
	if !sep(cur) {
		return expr;
	}

	let mut exprs = vec![expr, parse_item(cur)];
	while sep(cur) {
		exprs.push(parse_item(cur));
	}
	item(exprs)
}

fn parse_and(cur: &mut Cursor) -> Expr {
	parse_chain(cur, parse_expr_primary, |cur| cur.try_punct('&'), Expr::And)
}
fn parse_seq(cur: &mut Cursor) -> Expr {
	let sep = |cur: &mut Cursor| {
		!is_expr_end(cur) && !cur.test_punct('|') && !cur.test_multi_punct(['-', '>'])
	};
	parse_chain(cur, parse_and, sep, Expr::Seq)
}
fn parse_imply(cur: &mut Cursor) -> Expr {
	let expr = parse_seq(cur);
	if cur.try_multi_punct(['-', '>']) {
		Expr::Imply { cond: Box::new(expr), expr: Box::new(parse_seq(cur)) }
	} else {
		expr
	}
}

pub fn parse_expr(cur: &mut Cursor) -> Expr {
	parse_chain(cur, parse_imply, |cur| cur.try_punct('|'), Expr::Or)
}

/// matcher definition
///
/// **grammer**: ("for" -> matched_type:type ':') expr ("=>" -> map:expr))
#[derive(Debug, Clone)]
pub struct Matcher {
	pub matched_type: Option<TokenStream>,
	pub expr: Expr,
}
pub fn parse_matcher(cur: &mut Cursor, inside_call: bool) -> Matcher {
	let matched_type = match cur.try_kw("for") {
		true => {
			let ty = cur.eat_until("a type", |cur| {
				cur.try_multi_punct([':', ':']);
				cur.test_punct(':')
			});
			cur.punct(':');
			ty
		}
		false => None,
	};

	let mut expr = parse_expr(cur);
	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until("a expr", |cur| {
			if inside_call { cur.test_punct(',') || cur.test_punct('>') } else { cur.is_end() }
		}),
		false => None,
	};

	let ident = Ident::new("root", Span::call_site());
	let cap = Capture { ident, map, expr, ..Default::default() };
	expr = Expr::Capture(Box::new(cap));

	Matcher { matched_type, expr }
}

/// term optmize attribute
///
/// **grammer**: ('#' -> '[' "optimize" '(' list<"check" | "test" | "inline", ','> ')' ']')
#[derive(Debug, Clone, Default)]
pub struct TermOptimize {
	pub check: bool,
	pub test: bool,
	pub inline: bool,
}

/// a term in gramex macro
///
/// **grammer**: `
/// 	optimize:term_optimize "let" ident ('<' -> args:list<ident, ','> '>') (':' -> type)
/// 	'=' expr ("=>" -> map:expr)
/// `
#[derive(Debug, Clone)]
pub struct Term {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub optimize: TermOptimize,
	pub expr: Expr,
}

/// a grammer declaration
///
/// **grammer**: `
///   'for' matched_type:type ';' terms*:term
/// `
#[derive(Debug, Clone)]
pub struct GrammarDecl {
	pub matched_type: TokenStream,
	pub terms: Vec<Term>,
}

fn parse_term_optimize(cur: &mut Cursor) -> TermOptimize {
	let mut optmize = TermOptimize::default();
	if !cur.try_punct('#') {
		return optmize;
	};
	let Some(mut cur) = cur.enter_group(Bracket) else { return optmize };
	if cur.tokens.len() > 2 {
		cur.expected("`]`");
	}

	if cur.kw("optimize").is_some() {
		let Some(mut cur) = cur.enter_group(Parenthesis) else { return optmize };
		loop {
			if cur.try_kw("check") {
				optmize.check = true;
			} else if cur.try_kw("test") {
				optmize.test = true;
			} else if cur.try_kw("inline") {
				optmize.inline = true;
			} else {
				cur.expected("one of `check`, `test`, `inline`");
			}

			cur.try_eat_until(|cur| cur.try_punct(','));
			if cur.is_end() {
				break;
			}
		}
	}
	optmize
}

fn parse_term_args(cur: &mut Cursor) -> Vec<Ident> {
	let mut args = Vec::new();
	if cur.try_punct('<') {
		loop {
			cur.ident().map(|arg| args.push(arg));
			cur.try_eat_until(|cur| cur.try_punct(',') || cur.test_punct('>'));
			if cur.is_end() {
				cur.expected("`>`");
				break;
			}
			if cur.try_punct('>') {
				break;
			}
		}
	}
	args
}

fn try_parse_term(cur: &mut Cursor) -> Option<Term> {
	let optimize = parse_term_optimize(cur);
	if cur.kw("let").is_none() {
		cur.skip();
		return None;
	}

	let name = cur.ident().unwrap_or_else(|| Ident::new("_", Span::call_site()));
	let args = parse_term_args(cur);

	let ty = parse_capture_type(cur);
	cur.punct('=')?;
	let mut expr = parse_expr(cur);
	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until("an expression", |cur| cur.test_punct(';')),
		false => None,
	};

	let cap = Capture { ident: name.clone(), ty, map, expr, ..Default::default() };
	expr = Expr::Capture(Box::new(cap));
	cur.punct(';');

	Some(Term { name, args, optimize, expr })
}

pub fn parse_grammer_decl(cur: &mut Cursor) -> GrammarDecl {
	cur.kw("for");
	let unit_type = || quote!(());
	let matched_type = cur.eat_until("a type", |cur| cur.test_punct(';')).unwrap_or_else(unit_type);
	cur.punct(';');

	let mut terms = Vec::new();
	while !cur.is_end() {
		try_parse_term(cur).map(|t| terms.push(t));
	}
	GrammarDecl { matched_type, terms }
}
