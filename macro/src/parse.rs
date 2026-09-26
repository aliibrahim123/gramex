//! transform [`TokenStream`] into an AST

// this module incoperate alot of recovery strategies, failures doesnt stop parsing nor capture resolving, and recovered ast is used for resilient syntax highlighting.

// expressions are naivly consumed through `!','+` matcher for simplicity reasons, this work most of the time as commas are usually used inside groups, though top level generic path segment and closure arguments must be enclosed inside paranthesis.

use chunked_quote::{quote, token};
use proc_macro2::{
	Delimiter::{self, Brace, Bracket, Parenthesis},
	Group, Ident, Span, TokenStream, TokenTree,
};

use crate::{
	capture::CapInfo,
	cursor::{Cursor, Error, err, ident},
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
	pub fn is_exact(self) -> bool {
		self.0 == self.1
	}
}

/// a single mathcer
#[derive(Debug, Clone)]
pub enum Atom {
	/// literal, blocks, paths and ranges that resolve to a matcher
	///
	/// **grammer**: `literal | block | path | value_atom ".." value_atom`
	Matcher(TokenTree),
	/// match any item: `_`
	Any,
	/// enclosed expression,
	Group(Box<Expr>),
	/// call to compound matcher: `path '<' args:list<matcher, ','> '>'`
	Call { path: Box<[TokenTree]>, args: Box<[Matcher]> },
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

/// capture the matched section
///
/// **grammer**: `ident rep? ':' atom |  
///     '(' ident rep? (":" -> cap_type) '=' expr ("=>" -> map:expr) ')'
/// `
#[derive(Debug, Clone)]
pub struct Capture {
	pub ident: Ident,
	pub rep: Rep,
	pub ty: CapType,
	pub map: Option<TokenStream>,
	pub expr: Expr,
	/// resolved info by [`crate::capture`] module, is `None` on errors
	pub info: Option<CapInfo>,
}

impl Default for Capture {
	fn default() -> Self {
		Self {
			ident: ident!("default"),
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
	Capture(Box<Capture>),
	/// sequence of expressions: `expr+`
	Seq(Vec<Expr>),
	/// match any of expressions: `list<expr, '|'>`
	Or(Vec<Expr>),
	/// match all of the expressions: `list<expr, '&'>`
	And(Vec<Expr>),
	/// matches `expr` if `cond` matches, else match nothing: `expr "->" expr`
	Imply {
		cond: Box<Expr>,
		expr: Box<Expr>,
	},
	/// duppy placeholder for error encountered during parsing
	Error,
}

/// at an end of expression chain
fn is_expr_end(cur: &Cursor) -> bool {
	cur.is_end()
		|| cur.test_punct(',') // in call atom args
		|| cur.test_punct(';') // term end
		|| cur.test_punct('>') // call atom arg end
		|| cur.test_multi_punct(['=', '>']) // the map operator
		|| cur.test_kw("let") // to allow forgetten `;` recovery
}

/// try parse simple path: `"::"? list<ident, "::">`
fn try_parse_path(cur: &mut Cursor) -> Option<Vec<TokenTree>> {
	let start = cur.ind;
	let mut segments = Vec::new();

	if cur.try_multi_punct([':', ':']) {
		// append `::`
		segments.extend(cur.tokens[cur.ind - 2..cur.ind].iter().cloned());
	}

	let Some(first_ident) = cur.try_ident() else {
		cur.rewind(start);
		return None;
	};
	segments.push(first_ident.into());

	while cur.try_multi_punct([':', ':']) {
		let Some(ident) = cur.ident() else { break };
		// append `::`
		segments.extend(cur.tokens[cur.ind - 3..cur.ind - 1].iter().cloned());
		segments.push(ident.into());
	}

	Some(segments)
}

/// parse `'[' exact:nb ']' | '[' min?:nb ".." max?:nb ']'` [`Rep`]
#[allow(clippy::map_unwrap_or)]
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
/// parse [`Rep`]
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

/// parse `path | path '<' args:list<matcher, ','> '>'` [`Atom`]
fn parse_atom_path(cur: &mut Cursor, mut path: Vec<TokenTree>) -> Option<Atom> {
	if cur.try_punct('<') {
		let mut args = Vec::new();
		if !cur.test_punct('>') {
			args.push(parse_call_arg(cur));
			while cur.try_punct(',') && !cur.test_punct('>') {
				args.push(parse_call_arg(cur));
			}
		}
		cur.punct('>')?;
		Some(Atom::Call { path: path.into_boxed_slice(), args: args.into_boxed_slice() })
	} else if path.len() == 1 {
		Some(Atom::Matcher(path.pop().unwrap()))
	} else {
		Some(Atom::Matcher(TokenTree::Group(Group::new(
			Delimiter::None,
			TokenStream::from_iter(path),
		))))
	}
}
/// parse all [`Atom`] - [`Atom::Group`]
fn parse_atom_common(cur: &mut Cursor) -> Option<Atom> {
	if cur.try_kw("_") {
		Some(Atom::Any)
	} else if let Some(lit) = cur.try_literal() {
		Some(Atom::Matcher(lit.into()))
	} else if let Some(block) = cur.try_group(Brace) {
		let matcher = match block.stream().into_iter().next() {
			// transform clojure into `MathFn`
			Some(TokenTree::Punct(punct)) if punct.as_char() == '|' => {
				token!((::gramex::general::MatchFn::new_infer(#{block.stream()})))
			}
			_ => block.into(),
		};
		Some(Atom::Matcher(matcher))
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

/// parse capture type specifier
fn parse_capture_type(cur: &mut Cursor) -> CapType {
	if !cur.try_punct(':') {
		CapType::Inherited
	} else if cur.try_kw("struct") {
		CapType::Struct(cur.try_ident())
	} else if cur.try_kw("enum") {
		CapType::Enum(cur.try_ident())
	} else {
		cur.eat_until_non_empty("a type", |cur| cur.test_punct('='))
			.map_or(CapType::Inherited, CapType::Explicit)
	}
}

/// try parse inline capture variant `ident rep? ':' atom`
fn try_inline_capture(cur: &mut Cursor) -> Option<Expr> {
	let start = cur.ind;
	let ident = cur.try_ident()?;
	let rep = parse_rep(cur);

	// quard against paths, eating `:` for free
	if !cur.try_punct(':') || cur.test_punct(':') {
		cur.rewind(start);
		return None;
	}

	let expr = if let Some(atom) = parse_atom_common(cur) {
		Expr::Unit { not: false, near: false, atom, rep: Rep::ONCE }
	} else {
		Expr::Error
	};

	let cap = Capture { ident, rep, expr, ..Default::default() };
	Some(Expr::Capture(Box::new(cap)))
}
/// try parse regular capture variant
fn try_parse_capture(cur: &mut Cursor, flags_span: Option<Span>) -> Option<Expr> {
	let start = cur.ind;
	let ident = cur.try_ident()?;
	let rep = parse_rep(cur);

	// resolve disambiguaty between capture and group atom
	if !(cur.test_punct('=') || cur.test_punct(':') && !cur.test_multi_punct([':', ':']))
	{
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
		true => cur.eat_until_non_empty("an expression", |cur| cur.is_end()),
		false => None,
	};

	if !cur.is_end() {
		cur.expected("`)`");
	}

	let cap = Capture { ident, rep, ty, map, expr, info: None };
	Some(Expr::Capture(Box::new(cap)))
}

/// continue parsing a range atom after ".."
fn parse_range_atom(
	cur: &mut Cursor, atom: Atom, not: bool, near: bool, flags_span: Span,
) -> Expr {
	let Atom::Matcher(left) = atom else {
		err!(cur, "expected a value atom", flags_span);
		return Expr::Unit { not, near, rep: Rep::ONCE, atom };
	};
	if near | not {
		err!(cur, "range can not have modifiers", flags_span);
	}

	let right_span = cur.cur_span();
	let right = match parse_atom_common(cur) {
		Some(Atom::Matcher(right)) => right,
		Some(atom) => {
			err!(cur, "expected a value atom", right_span);
			return Expr::Seq(vec![
				Expr::Unit { not, near, rep: Rep::ONCE, atom: Atom::Matcher(left) },
				Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom },
			]);
		}
		None => {
			// an error is raised by `parse_atom`
			return Expr::Unit { not, near, rep: Rep::ONCE, atom: Atom::Matcher(left) };
		}
	};

	let atom = Atom::Matcher(token! { (#{left}..=#{right}) });
	Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom }
}

/// parse top level expressions: unit, captures and errors
fn parse_expr_primary(cur: &mut Cursor) -> Expr {
	if let Some(expr) = try_inline_capture(cur) {
		return expr;
	}

	let flags_span = cur.cur_span();
	let not = cur.try_punct('!');
	let near = cur.try_punct('~');

	let atom = if let Some(mut cur) = cur.try_enter_group(Parenthesis) {
		if let Some(expr) =
			try_parse_capture(&mut cur, (not | near).then_some(flags_span))
		{
			return expr;
		}

		let expr = parse_expr(&mut cur);
		if !cur.is_end() {
			cur.expected("`)`");
		}
		Atom::Group(Box::new(expr))
	} else if let Some(atom) = parse_atom_common(cur) {
		atom
	} else {
		return Expr::Error;
	};

	if cur.try_multi_punct(['.', '.']) {
		parse_range_atom(cur, atom, not, near, flags_span)
	} else {
		let rep = parse_rep(cur);
		Expr::Unit { not, near, rep, atom }
	}
}

// parse `expr | (exprs = list<expr, sep> => construct(exprs))`
fn parse_chain(
	cur: &mut Cursor, expr: impl Fn(&mut Cursor) -> Expr,
	sep: impl Fn(&mut Cursor) -> bool, construct: impl Fn(Vec<Expr>) -> Expr,
) -> Expr {
	let first = expr(cur);
	if !sep(cur) {
		return first;
	}

	let mut exprs = vec![first, expr(cur)];
	while sep(cur) {
		exprs.push(expr(cur));
	}
	construct(exprs)
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
fn parse_or(cur: &mut Cursor) -> Expr {
	parse_chain(cur, parse_imply, |cur| cur.try_punct('|'), Expr::Or)
}

/// parse an [`Expr`]
pub fn parse_expr(cur: &mut Cursor) -> Expr {
	parse_or(cur)
}

/// matcher definition
#[derive(Debug, Clone)]
pub struct Matcher {
	pub matched_type: Option<TokenStream>,
	pub cap: Capture,
}
/// parse a [`Matcher`] argument in a call atom
///
/// **grammer**: `("for" -> `matched_type:type` ':') expr ("=>" -> map:expr))
pub fn parse_call_arg(cur: &mut Cursor) -> Matcher {
	let matched_type = match cur.try_kw("for") {
		true => {
			let ty = cur.eat_until_non_empty("a type", |cur| {
				// allow path types
				cur.try_multi_punct([':', ':']);
				cur.test_punct(':')
			});
			cur.punct(':');
			ty
		}
		false => None,
	};

	let expr = parse_expr(cur);
	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until_non_empty("a expression", |cur| {
			cur.test_punct(',') || cur.test_punct('>')
		}),
		false => None,
	};

	let cap = Capture { ident: ident!("root"), map, expr, ..Default::default() };

	Matcher { matched_type, cap }
}

/// parse a [`Matcher`] for `matcher` macro
///
/// grammar: `"for" matched:type ':' ("Capture" -> ':' type)? expr ("=>" -> map:expr)`
pub fn parse_matcher(cur: &mut Cursor) -> Matcher {
	if cur.kw("for").is_none() {
		return Matcher { matched_type: None, cap: Capture::default() };
	}
	let matched_type = cur.eat_until_non_empty("a type", |cur| cur.test_punct(','));
	cur.punct(',');

	let cap_type = match cur.try_kw("Capture") {
		true => {
			cur.punct(':');
			let ty = cur.eat_until_non_empty("a type", |cur| cur.test_punct(','));
			cur.punct(',');
			ty.map_or(CapType::Inherited, CapType::Explicit)
		}
		false => CapType::Inherited,
	};

	let expr = parse_expr(cur);
	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until_non_empty("a expression", |cur| cur.is_end()),
		false => None,
	};
	if !cur.is_end() {
		err!(cur, "expected end of input");
	}

	let cap =
		Capture { ident: ident!("root"), expr, ty: cap_type, map, ..Default::default() };

	Matcher { matched_type, cap }
}

/// a term in gramex macro
///
/// **grammer**: `
/// "let" ident ('<' -> args:list<ident, ','> '>') (':' -> type)
/// '=' expr ("=>" -> map:expr)
/// `
#[derive(Debug, Clone)]
pub struct Term {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub cap: Capture,
}

/// a grammer declaration
///
/// **grammer**: `"for" matched_type:type ';' terms*:term`
#[derive(Debug, Clone)]
pub struct GrammarDecl {
	pub matched_type: TokenStream,
	pub terms: Vec<Term>,
}
fn parse_term_args(cur: &mut Cursor) -> Vec<Ident> {
	let mut args = Vec::new();
	if cur.try_punct('<') {
		loop {
			if let Some(arg) = cur.ident() {
				args.push(arg);
			}
			cur.eat_until(|cur| cur.try_punct(',') || cur.test_punct('>'));
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
/// parse a [`Term`]
fn try_parse_term(cur: &mut Cursor) -> Option<Term> {
	if cur.kw("let").is_none() {
		cur.skip();
		return None;
	}

	let name = cur.ident().unwrap_or_else(|| ident!("_"));
	let args = parse_term_args(cur);

	let ty = parse_capture_type(cur);
	cur.punct('=')?;
	let expr = parse_expr(cur);

	let map = match cur.try_multi_punct(['=', '>']) {
		true => cur.eat_until_non_empty("an expression", |cur| cur.test_punct(';')),
		false => None,
	};
	let cap = Capture { ident: name.clone(), ty, map, expr, ..Default::default() };

	cur.punct(';');
	Some(Term { name, args, cap })
}

/// parse a [`GrammarDecl`]
pub fn parse_grammer_decl(cur: &mut Cursor) -> GrammarDecl {
	cur.kw("for");
	let matched_type = cur
		.eat_until_non_empty("a type", |cur| cur.test_punct(';'))
		.unwrap_or_else(|| quote!(()));
	cur.punct(';');

	let mut terms = Vec::new();
	while !cur.is_end() {
		if let Some(t) = try_parse_term(cur) {
			terms.push(t);
		}
	}
	GrammarDecl { matched_type, terms }
}

/// match expression macros args
///
/// grammer: `(when(!is_cur_op)} 'for' -> matched_type:type ',') value:expr ',' expr`
#[derive(Debug, Clone)]
pub struct MatchExpr {
	pub matched_type: Option<TokenStream>,
	pub value: TokenStream,
	pub expr: Expr,
}

/// parse a [`MatchExpr`]
pub fn parse_match_expr(cur: &mut Cursor, is_cur_op: bool) -> Option<MatchExpr> {
	let matched_type = if !is_cur_op && cur.try_kw("for") {
		let ty = cur.eat_until_non_empty("a type", |cur| cur.test_punct(','));
		cur.punct(',');
		ty
	} else {
		None
	};

	let value = cur.eat_until_non_empty("an expression", |cur| cur.test_punct(','))?;
	cur.punct(',');

	let expr = parse_expr(cur);

	if !cur.is_end() {
		cur.expected("end of input");
	}
	Some(MatchExpr { matched_type, value, expr })
}

/// `match_map` macro args
///
/// grammer: `cursor:ident ',' '{'
///     (arms* = pat:expr "=>" map:expr") ("else" -> "=>" else_:expr) ','?
/// '}'`
#[derive(Debug)]
pub struct MatchMap {
	pub cursor: Ident,
	pub arms: Vec<Capture>,
	pub else_: TokenStream,
}

/// parse `pat:expr "=>" map:expr` in `match_map` body
fn parse_match_map_arms(cur: &mut Cursor) -> Vec<Capture> {
	let mut arms = Vec::new();
	while !cur.is_end() && !cur.test_kw("else") {
		let pat = parse_expr(cur);
		cur.multi_punct(['=', '>']);

		let map = cur.eat_until_non_empty("an expression", |cur| cur.test_punct(','));

		// transform into a capture to simplify codegen
		let cap = Capture { ident: ident!("root"), expr: pat, map, ..Default::default() };
		arms.push(cap);

		if !cur.is_end() {
			cur.punct(',');
		}
	}
	arms
}

/// parse a [`MatchMap`]
pub fn parse_match_map(cur: &mut Cursor) -> Option<MatchMap> {
	let cursor = cur.ident()?;
	cur.punct(',');

	let mut arms_cur = cur.enter_group(Brace)?;
	let arms = parse_match_map_arms(&mut arms_cur);

	let else_ = if arms_cur.try_kw("else") {
		arms_cur.multi_punct(['=', '>']);
		let expr = arms_cur
			.eat_until_non_empty("an expression", |cur| cur.test_punct(','))
			.unwrap_or_else(|| quote!(core::unreachable!()));
		arms_cur.try_punct(',');
		expr
	} else {
		quote! { core::unreachable!() }
	};

	if !cur.is_end() {
		cur.expected("end of input");
	}
	Some(MatchMap { cursor, arms, else_ })
}

/// `derive_enum_matcher` macro args
///
/// grammer: `
///     '#' '[' "derive_enum_matcher" '('
///         "for" matched_type:type (',' "field" '=' field:expr)?
///         (',' "expected" '=' expected:expr)?
///     ')' ']'
///     "enum" name:ident "{" vars:list<variant, ','> "}"
/// `  
#[derive(Debug)]
pub struct EnumMatcher {
	pub name: Ident,
	pub matched_type: TokenStream,
	pub field: TokenStream,
	pub expected: Option<TokenStream>,
	pub vars: Vec<Variant>,
}

/// enum variant
///
/// grammer: `name:ident` ('(' fields:list<type, ','> ')')? !','*
#[derive(Debug)]
pub struct Variant {
	pub name: Ident,
	pub fields: Option<Vec<TokenStream>>,
}

/// skip generic arguments
fn skip_generics(cur: &mut Cursor) -> bool {
	if !cur.try_punct('<') {
		return false;
	}
	// using angle bracket balancing
	let mut angle_count = 1;
	while !(angle_count == 0 || cur.is_end()) {
		if cur.test_punct('<') {
			angle_count += 1;
		} else if cur.test_punct('>') {
			angle_count -= 1;
		}
		cur.skip();
	}
	true
}

/// parse enum variant fields
fn parse_var_fields(mut cur: Cursor) -> Vec<TokenStream> {
	let mut fields = Vec::new();
	let mut last_ind = 0;
	while !cur.is_end() {
		if cur.test_punct(',') {
			fields.push(cur.tokens[last_ind..cur.ind].iter().cloned().collect());
			cur.skip();
			last_ind = cur.ind;
		} else if !skip_generics(&mut cur) {
			cur.skip();
		}
	}
	if last_ind < cur.tokens.len() {
		fields.push(cur.tokens[last_ind..].iter().cloned().collect());
	}
	fields
}

/// parse enum [`Variant`]s
fn parse_enum_variants(cur: &mut Cursor) -> Option<Vec<Variant>> {
	let mut vars_cur = cur.enter_group(Brace)?;
	let mut vars = Vec::new();
	while !vars_cur.is_end() {
		if let Some(name) = vars_cur.ident() {
			let fields = vars_cur.try_enter_group(Parenthesis).map(parse_var_fields);
			vars.push(Variant { name, fields });
		}
		// skip the rest of variant syntax
		vars_cur.eat_until(|cur| cur.try_punct(','));
	}
	Some(vars)
}

/// parse enum [`EnumMatcher`] attribute meta
fn parse_enum_matcher_attr(
	cur: &mut Cursor,
) -> (TokenStream, TokenStream, Option<TokenStream>) {
	cur.kw("for");
	let matched_type = cur
		.eat_until_non_empty("a type", |cur| {
			if skip_generics(cur) {
				cur.ind -= 1;
			}
			cur.test_punct(',')
		})
		.unwrap_or_default();
	cur.try_punct(',');

	let mut field = TokenStream::new();
	if cur.try_kw("field") {
		cur.punct('=');
		field = cur
			.eat_until_non_empty("an expression", |cur| cur.test_punct(','))
			.unwrap_or_default();
	}
	cur.try_punct(',');
	let expected = cur.try_kw("expected").then(|| {
		cur.punct('=');
		cur.eat_until_non_empty("a path", |_| false)
	});

	if !cur.is_end() {
		cur.expected("end of input");
	}

	(matched_type, field, expected.flatten())
}

/// parse [`EnumMatcher`]
pub fn parse_enum_matcher(
	attr: TokenStream, item: TokenStream, errors: &mut Vec<Error>,
) -> Option<EnumMatcher> {
	let mut attr_cur = Cursor::new(attr, Span::call_site(), errors);
	let (matched_type, field, expected) = parse_enum_matcher_attr(&mut attr_cur);

	let mut item_cur = Cursor::new(item, Span::call_site(), errors);
	// skip attrs and visibility
	while item_cur.try_punct('#') {
		item_cur.skip();
	}
	if item_cur.try_kw("pub") {
		item_cur.try_group(Parenthesis);
	}

	if item_cur.kw("enum").is_none() {
		err!(item_cur, "`derive_enum_matcher` attribute can only be applied to enums");
		return None;
	}

	let name = item_cur.ident()?;
	skip_generics(&mut item_cur);

	let vars = parse_enum_variants(&mut item_cur)?;
	Some(EnumMatcher { name, matched_type, field, expected, vars })
}
