use std::mem;

use chunked_quote::{chunk, quote};
use proc_macro2::{Delimiter, Group, Ident, TokenStream};
use quote::ToTokens;
use rustc_hash::FxHashSet;

use crate::{
	cursor::{Error, err},
	parse::{Atom, CapType, Capture, Expr, GrammarDecl, Matcher, Rep, Term},
};

#[derive(Debug, Clone)]
pub enum CapKind {
	Normal { need_from: bool },
	Atomic { need_from: bool },
	Tuple(Vec<Ident>),
	Struct(Vec<Ident>),
	Enum(Vec<Option<Ident>>),
}
#[derive(Debug, Clone, Copy)]
pub enum CapContainer {
	None,
	Option,
	Vec,
	OptionVec,
}
impl CapContainer {
	pub fn wrap_type(self, mut result: &mut TokenStream, item: impl ToTokens) {
		match self {
			Self::None => item.to_tokens(result),
			Self::Option => chunk!(result, Option<#item> ),
			Self::Vec => chunk!(result, Vec<#item> ),
			Self::OptionVec => chunk!(result, Option<Vec<#item>> ),
		}
	}
}
#[derive(Debug, Clone)]
pub struct CapInfo {
	pub resolved_type: TokenStream,
	pub kind: CapKind,
	pub container: CapContainer,
}

#[derive(Debug, Clone)]
struct CapChild {
	name: Ident,
	resolved_type: TokenStream,
	container: CapContainer,
}

#[derive(Debug, Clone)]
struct CapParent {
	is_generated: bool,
	children: Vec<CapChild>,
	child_names: FxHashSet<String>,
}
impl CapParent {
	fn new(is_generated: bool) -> Self {
		Self { is_generated, children: Vec::new(), child_names: FxHashSet::default() }
	}
}

#[derive(Debug, Default)]
pub struct CapMod {
	pub stream: TokenStream,
	items: FxHashSet<String>,
}

#[derive(Debug)]
pub struct Context<'a> {
	pub capture_mod: Option<&'a mut CapMod>,
	pub matched_type: Option<&'a TokenStream>,
	pub errors: &'a mut Vec<Error>,
}

fn resolve_captures(
	expr: &mut Expr, is_optional: bool, parent: &mut CapParent, ctx: &mut Context<'_>,
) {
	match expr {
		Expr::And(exprs) | Expr::Seq(exprs) => {
			for expr in exprs {
				resolve_captures(expr, is_optional, parent, ctx);
			}
		}
		Expr::Imply { cond, expr } => {
			resolve_captures(cond, true, parent, ctx);
			resolve_captures(expr, true, parent, ctx);
		}
		Expr::Or(exprs) => {
			for expr in exprs {
				resolve_captures(expr, true, parent, ctx);
			}
		}
		Expr::Unit { not, rep, atom: Atom::Group(expr), .. } => {
			if *not || *rep != Rep::ONCE && *rep != Rep::OPTIONAL {
				forbid_captures(expr, ctx.errors);
			} else {
				resolve_captures(expr, (*rep == Rep::OPTIONAL) | is_optional, parent, ctx);
			};
		}
		Expr::Unit { atom: Atom::Call { .. }, .. } => forbid_captures(expr, ctx.errors),
		Expr::Capture(cap) => {
			_ = resolve_capture(&mut *cap, is_optional, parent, ctx);
		}
		_ => {}
	}
}
fn forbid_captures(expr: &Expr, errors: &mut Vec<Error>) {
	match expr {
		Expr::And(exprs) | Expr::Seq(exprs) | Expr::Or(exprs) => {
			for expr in exprs {
				forbid_captures(expr, errors);
			}
		}
		Expr::Imply { cond, expr } => {
			forbid_captures(cond, errors);
			forbid_captures(expr, errors);
		}
		Expr::Unit { atom: Atom::Group(expr), .. } => forbid_captures(expr, errors),
		Expr::Unit { atom: Atom::Call { args, .. }, .. } => {
			for arg in args {
				let root_expr =
					if let Expr::Capture(cap) = &arg.expr { &cap.expr } else { &arg.expr };
				forbid_captures(root_expr, errors);
			}
		}
		Expr::Capture(cap) => {
			errors.push(Error::new("capture not allowed here".to_string(), cap.ident.span()))
		}
		_ => {}
	}
}

fn has_capture(expr: &Expr) -> bool {
	match expr {
		Expr::And(exprs) | Expr::Seq(exprs) | Expr::Or(exprs) => exprs.iter().any(has_capture),
		Expr::Imply { cond, expr } => has_capture(cond) || has_capture(expr),
		Expr::Unit { atom: Atom::Group(expr), .. } => has_capture(expr),
		Expr::Unit { atom: Atom::Call { args, .. }, .. } => {
			args.iter().any(|arg| has_capture(&arg.expr))
		}
		Expr::Capture(_) => true,
		_ => false,
	}
}

fn default_cap(matched_type: Option<&TokenStream>) -> TokenStream {
	if let Some(m) = matched_type {
		quote! { <#m as ::gramex::MatchAble>::Slice<'a> }
	} else {
		TokenStream::new()
	}
}

#[derive(Debug)]
enum Create {
	None,
	Struct(Ident),
	Enum(Ident),
}

fn resolve_capture_type(cap: &mut Capture, ctx: &mut Context) -> Result<(TokenStream, Create), ()> {
	let mut resolve_gen_item = |item_ident: &mut Option<_>, create: fn(_) -> _| -> Result<_, ()> {
		let item_ident = item_ident.take().unwrap_or_else(|| pascal_case(&cap.ident));

		let Some(cap_mod) = ctx.capture_mod.as_deref_mut() else {
			let msg = "can not use generated capture type outside grammar declerations";
			err!(ctx, msg, cap.ident.span());
			return Err(());
		};
		if !cap_mod.items.insert(item_ident.to_string()) {
			err!(ctx, "a generated item exist with the same name", item_ident.span());
			return Err(());
		}

		Ok((quote! { captures::#item_ident<'a> }, create(item_ident)))
	};

	match &mut cap.ty {
		CapType::Inherited => Ok((default_cap(ctx.matched_type), Create::None)),
		CapType::Explicit(ty) => Ok((ty.clone(), Create::None)),
		CapType::Struct(item_ident) => resolve_gen_item(item_ident, Create::Struct),
		CapType::Enum(item_ident) => resolve_gen_item(item_ident, Create::Enum),
	}
}

fn life_marker(stream: &mut TokenStream) {
	chunk!(stream, (::std::marker::PhantomData<&'a ()>, ::std::convert::Infallible));
}

fn resolve_struct_capture(
	cap: &mut Capture, resolved_type: &mut TokenStream, create: Create, parent: &CapParent,
	ctx: &mut Context,
) -> Result<CapKind, ()> {
	let mut _self = CapParent::new(matches!(create, Create::Struct(_)));
	resolve_captures(&mut cap.expr, false, &mut _self, ctx);

	if let Create::Struct(item) = create {
		let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
		chunk!(stream,
			# #[derive(Debug)]
			pub struct #item<'a> {
				#for CapChild { name, resolved_type, container } in &_self.children #{
					pub #name: #do {container.wrap_type(stream, resolved_type)},
				}
				# #[doc(hidden)] pub __life_marker: #do {life_marker(stream)
			},
		})
	} else if let Create::Enum(_) = create {
		err!(ctx, "expected root or expression for generated enum", cap.ident.span());
		return Err(());
	}

	let fields = |_self: CapParent| _self.children.into_iter().map(|cap| cap.name).collect();
	if matches!(cap.ty, CapType::Inherited) {
		if parent.is_generated {
			*resolved_type = quote! { ( #for child in &_self.children #{
				#do {child.container.wrap_type(__stream, &child.resolved_type)},
			})}
		}
		Ok(CapKind::Tuple(fields(_self)))
	} else {
		Ok(CapKind::Struct(fields(_self)))
	}
}

fn resolve_enum_variant(
	expr: &mut Expr, variant_names: &mut FxHashSet<String>, variants_def: &mut Option<TokenStream>,
	ctx: &mut Context,
) -> Option<Ident> {
	let mut parent = CapParent::new(true);
	resolve_captures(expr, false, &mut parent, ctx);

	if parent.children.len() > 1 {
		let msg = "an or branch in a capture enum must have at most one capture";
		err!(ctx, msg, parent.children[0].name.span());
	}

	let child = parent.children.drain(..).next();

	if let Some(CapChild { name, resolved_type, container }) = &child {
		if !variant_names.insert(name.to_string()) {
			err!(ctx, "a variant exist with the same name", name.span());
			return None;
		}

		if let Some(mut def) = variants_def.as_mut() {
			chunk!(def, #{pascal_case(name)}(#do {container.wrap_type(def, resolved_type)}),)
		}
	}
	child.map(|c| c.name)
}

fn resolve_enum_capture(
	exprs: &mut [Expr], ident: &Ident, create: Create, ctx: &mut Context,
) -> Result<CapKind, ()> {
	let mut variants = Vec::new();
	let mut variant_names = FxHashSet::default();
	let mut variants_def = matches!(create, Create::Enum(_)).then(TokenStream::new);
	let mut has_none = false;
	for expr in exprs {
		let var = resolve_enum_variant(expr, &mut variant_names, &mut variants_def, ctx);
		has_none |= var.is_none();
		variants.push(var);
	}

	if let Create::Enum(item) = create {
		let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
		chunk!(stream,
			# #[derive(Debug)]
			pub enum #item<'a> {
				#if has_none #{ None, }
				#do {stream.extend(variants_def.unwrap())}
				# #[doc(hidden)] __LifeMarker #do {life_marker(stream)
			},
		})
	} else if let Create::Struct(_) = create {
		err!(ctx, "expected root non or expression for generated struct", ident.span());
		return Err(());
	}

	Ok(CapKind::Enum(variants))
}

fn is_atomic_unit(expr: &Expr) -> bool {
	matches!(expr,
		Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom }
		if matches!(atom, Atom::Matcher(_) | Atom::Call { .. })
	) || matches!(expr, Expr::Range(_, _))
}

fn resolve_leaf_capture(
	cap: &mut Capture, create: Create, ctx: &mut Context,
) -> Result<CapKind, ()> {
	let need_from = matches!(cap.ty, CapType::Inherited);
	Ok(match create {
		Create::Struct(item) => {
			let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
			chunk!(stream,
				# #[derive(Debug)]
				pub struct #item<'a> (#{default_cap(ctx.matched_type)});
			);
			CapKind::Struct(Vec::new())
		}
		Create::Enum(ident) => {
			err!(ctx, "expected root or expression for generated enum", ident.span());
			return Err(());
		}
		_ if is_atomic_unit(&cap.expr) => {
			if let Expr::Unit { atom: Atom::Call { args, .. }, .. } = &mut cap.expr {
				for arg in args {
					analyze_matcher(&mut *arg, ctx);
				}
			}
			CapKind::Atomic { need_from }
		}
		_ => CapKind::Normal { need_from },
	})
}

fn add_capture_child(
	cap: &Capture, resolved_type: &TokenStream, container: CapContainer, parent: &mut CapParent,
	ctx: &mut Context,
) -> Result<(), ()> {
	if !parent.child_names.insert(cap.ident.to_string()) {
		err!(ctx, "a sibling capture exist with the same name", cap.ident.span());
		return Err(());
	}
	parent.children.push(CapChild {
		name: cap.ident.clone(),
		resolved_type: resolved_type.clone(),
		container,
	});
	Ok(())
}

fn resolve_capture(
	cap: &mut Capture, is_optional: bool, parent: &mut CapParent, ctx: &mut Context,
) -> Result<(), ()> {
	let (mut resolved_type, create) = resolve_capture_type(cap, ctx)?;

	let kind = if has_capture(&cap.expr) && !is_atomic_unit(&cap.expr) {
		match &mut cap.expr {
			Expr::Or(exprs) => resolve_enum_capture(exprs, &cap.ident, create, ctx)?,
			_ => resolve_struct_capture(cap, &mut resolved_type, create, parent, ctx)?,
		}
	} else {
		resolve_leaf_capture(cap, create, ctx)?
	};

	let container = match (is_optional, cap.rep) {
		(false, Rep::ONCE) => CapContainer::None,
		(true, Rep::ONCE) | (_, Rep::OPTIONAL) => CapContainer::Option,
		(false, _) => CapContainer::Vec,
		(true, _) => CapContainer::OptionVec,
	};

	add_capture_child(cap, &resolved_type, container, parent, ctx)?;

	let info = CapInfo { container, resolved_type, kind };
	cap.info = Some(info);

	Ok(())
}

fn pascal_case(ident: &Ident) -> Ident {
	let orig = ident.to_string();
	let mut res = String::with_capacity(orig.len());
	for section in orig.split('_') {
		if let Some(char) = section.chars().next() {
			res.push(char.to_ascii_uppercase());
		}
		section.get(1..).map(|s| res.push_str(s));
	}
	Ident::new(&res, ident.span())
}

#[derive(Debug, Clone)]
pub struct AnalyzeResult {
	pub has_captures: bool,
}

pub fn analyze_matcher(matcher: &mut Matcher, ctx: &mut Context) -> AnalyzeResult {
	let mut ctx = Context {
		matched_type: matcher.matched_type.as_ref().or(ctx.matched_type),
		capture_mod: ctx.capture_mod.as_deref_mut(),
		errors: ctx.errors,
	};

	let root_expr = if let Expr::Capture(cap) = &matcher.expr { &cap.expr } else { &matcher.expr };
	if has_capture(root_expr) {
		resolve_captures(&mut matcher.expr, false, &mut CapParent::new(false), &mut ctx);
		AnalyzeResult { has_captures: true }
	} else {
		AnalyzeResult { has_captures: false }
	}
}

pub fn analyze_term(term: &mut Term, ctx: &mut Context) -> AnalyzeResult {
	let Term { args, expr, .. } = term;
	let mut arg_names = FxHashSet::default();
	for i in 0..args.len() {
		if !arg_names.insert(args[i].to_string()) {
			err!(ctx, "another argument exist with the same name", args[i].span());
			args[i] = Ident::new("_", args[i].span());
		}
	}

	let Expr::Capture(root_cap) = &expr else { unreachable!() };
	if !matches!(root_cap.ty, CapType::Inherited)
		|| root_cap.map.is_some()
		|| has_capture(&root_cap.expr)
	{
		resolve_captures(expr, false, &mut CapParent::new(true), ctx);
		AnalyzeResult { has_captures: true }
	} else {
		AnalyzeResult { has_captures: false }
	}
}
