//! symantic analysis for captures: validation, type resolution, item generation, kind resolution..

use chunked_quote::{chunk, quote};
use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use rustc_hash::FxHashSet;

use crate::{
	cursor::{Error, err},
	parse::{Atom, CapType, Capture, Expr, Matcher, Rep, Term},
};

/// the kind of the capture
#[derive(Debug, Clone)]
pub enum CapKind {
	/// `(cap = "abc")` => `matched.slice()`
	Slice { need_from: bool },
	/// `cap:atom` => `atom.capture()`
	Atomic { need_from: bool },
	/// `(cap = a:'b' c:'d')` => `(a, c)`
	Tuple(Vec<CapChild>),
	/// `(cap: SomeStruct = a:'b' c:'d')` => `SomeStruct { a, d }`
	Struct { fields: Vec<CapChild>, is_generated: bool },
	/// `(cap = a:'b' c:'d' => [a, b])` => `[a, b]`
	ReduceMap(Vec<CapChild>),
	/// `(cap: struct = "abc")` => `Cap(value.slice())`
	UnitStruct,
	/// `(cap: SomeEnum = a:'b' | 'c')` => `SomeEnum::A | SomeEnum::None`
	///
	/// each branch gets its own slot, `None` if no capture
	Enum(Vec<Option<CapChild>>),
}

/// capture repetition container
#[derive(Debug, Clone, Copy)]
pub enum CapContainer {
	/// no container
	None,
	/// `Option<T>` for `cap?:_`
	Option,
	/// `Vec<T>` for `cap*:_ | cap[2..4]:_ | (_ cap+:_)?`
	Vec,
}
impl CapContainer {
	/// wrap `item` with the container in type position
	pub fn wrap_type(self, mut result: &mut TokenStream, item: impl ToTokens) {
		match self {
			Self::None => item.to_tokens(result),
			Self::Option => chunk!(result, __::Option<#item> ),
			Self::Vec => chunk!(result, __::Vec<#item> ),
		}
	}
}

/// resolved info about a capture during semantic analysis
#[derive(Debug, Clone)]
pub struct CapInfo {
	pub resolved_type: TokenStream,
	pub kind: CapKind,
	pub container: CapContainer,
}

/// nested capture info
#[derive(Debug, Clone)]
pub struct CapChild {
	pub name: Ident,
	pub resolved_type: TokenStream,
	pub container: CapContainer,
}
impl CapChild {
	fn write_type(&self, stream: &mut TokenStream) {
		self.container.wrap_type(stream, &self.resolved_type);
	}
}

/// capture parent info
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

/// capture module result
#[derive(Debug, Default)]
pub struct CapMod {
	pub stream: TokenStream,
	items: FxHashSet<String>,
}

/// analysis common state
#[derive(Debug)]
pub struct Context<'src> {
	/// only `Some` in grammer decl
	pub capture_mod: Option<&'src mut CapMod>,
	pub matched_type: Option<&'src TokenStream>,
	pub errors: &'src mut Vec<Error>,
}
impl<'src> Context<'src> {
	pub fn new_expr(
		matched_type: Option<&'src TokenStream>, errors: &'src mut Vec<Error>,
	) -> Context<'src> {
		Self { capture_mod: None, matched_type, errors }
	}
}

/// recursivly resolve info for all captures in the expression tree
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
		Expr::Unit { .. } => resolve_captures_unit(expr, is_optional, parent, ctx),
		Expr::Capture(cap) => {
			_ = resolve_capture(&mut *cap, is_optional, parent, ctx);
		}
		Expr::Error => {}
	}
}
/// resolve captures inside a unit expression
fn resolve_captures_unit(
	expr: &mut Expr, is_optional: bool, parent: &mut CapParent, ctx: &mut Context<'_>,
) {
	match expr {
		Expr::Unit { not, rep, atom: Atom::Group(expr), .. } => {
			if *not || *rep != Rep::ONCE && *rep != Rep::OPTIONAL {
				forbid_captures(expr, ctx.errors);
			} else {
				let is_optional = is_optional || (*rep == Rep::OPTIONAL);
				resolve_captures(expr, is_optional, parent, ctx);
			}
		}
		Expr::Unit { not, rep, atom: Atom::Call { args, .. }, .. } => {
			if *not || *rep != Rep::ONCE && *rep != Rep::OPTIONAL {
				for arg in args {
					forbid_captures(&arg.cap.expr, ctx.errors);
				}
			} else {
				for arg in args {
					analyze_matcher(arg, ctx);
				}
			}
		}
		_ => {}
	}
}

/// forbid captures in the whole expression tree
pub fn forbid_captures(expr: &Expr, errors: &mut Vec<Error>) {
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
				forbid_captures(&arg.cap.expr, errors);
			}
		}
		Expr::Capture(cap) => errors
			.push(Error::new("capture not allowed here".to_string(), cap.ident.span())),
		_ => {}
	}
}

/// check for capture presence in a expression tree
fn has_capture(expr: &Expr) -> bool {
	match expr {
		Expr::And(exprs) | Expr::Seq(exprs) | Expr::Or(exprs) => {
			exprs.iter().any(has_capture)
		}
		Expr::Imply { cond, expr } => has_capture(cond) || has_capture(expr),
		Expr::Unit { atom: Atom::Group(expr), .. } => has_capture(expr),
		Expr::Unit { atom: Atom::Call { args, .. }, .. } => {
			args.iter().any(|arg| has_capture(&arg.cap.expr))
		}
		Expr::Capture(_) => true,
		_ => false,
	}
}

/// the default capture type
fn default_cap(matched_type: Option<&TokenStream>) -> TokenStream {
	if let Some(m) = matched_type {
		quote! { <#m as __MatchAble>::Slice<'src> }
	} else {
		// should not do any harm as it is used only in matchers and generated items that are always has matched_type specified
		TokenStream::new()
	}
}

/// generated item creation request
#[derive(Debug)]
enum Create {
	None,
	Struct(Ident),
	Enum(Ident),
}

/// resolve generated item type
fn resolve_gen_type(
	item_ident: &mut Option<Ident>, cap_ident: &Ident, create: fn(Ident) -> Create,
	ctx: &mut Context,
) -> Result<(TokenStream, Create), ()> {
	let item_ident = item_ident.take().unwrap_or_else(|| pascal_case(cap_ident));

	let Some(cap_mod) = ctx.capture_mod.as_deref_mut() else {
		let msg = "can not use generated capture type outside grammar declerations";
		err!(ctx, msg, cap_ident.span());
		return Err(());
	};
	if !cap_mod.items.insert(item_ident.to_string()) {
		err!(ctx, "a generated item exist with the same name", item_ident.span());
		return Err(());
	}

	Ok((quote! { #item_ident::<'src> }, create(item_ident)))
}

/// resolve capture type from its type specifier
fn resolve_capture_type(
	cap: &mut Capture, ctx: &mut Context,
) -> Result<(TokenStream, Create), ()> {
	match &mut cap.ty {
		CapType::Inherited => Ok((default_cap(ctx.matched_type), Create::None)),
		CapType::Explicit(ty) => Ok((ty.clone(), Create::None)),
		CapType::Struct(item_ident) => {
			resolve_gen_type(item_ident, &cap.ident, Create::Struct, ctx)
		}
		CapType::Enum(item_ident) => {
			resolve_gen_type(item_ident, &cap.ident, Create::Enum, ctx)
		}
	}
}

/// generate generated struct definition
fn gen_struct(item: &Ident, this: &CapParent, ctx: &mut Context) {
	let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
	chunk!(stream,
		# #[derive(Debug)]
		pub struct #item<'src> {
			#for child in &this.children #{
				pub #{&child.name}: #do { child.write_type(stream) },
			}
			// case every capture is owned
			# #[doc(hidden)] pub __life_marker: __::PhantomData<&'src ()>,
		}
	);
}
/// generate generated unit struct definition
fn gen_unit_struct(item: &Ident, ctx: &mut Context) {
	let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
	chunk!(stream,
		# #[derive(Debug)]
		pub struct #item<'src> (pub #{default_cap(ctx.matched_type)});
	);
}
/// generate generated enum definition
fn gen_enum(
	item: &Ident, has_none: bool, variants_def: Option<TokenStream>, ctx: &mut Context,
) {
	let mut stream = &mut ctx.capture_mod.as_mut().unwrap().stream;
	chunk!(stream,
		# #[derive(Debug)]
		pub enum #item<'src> {
			#if has_none #{ None, }
			#do { stream.extend(variants_def.unwrap()) }
			// maybe all variants be owned, Infalliable to remove from exhustive check
			# #[doc(hidden)] __LifeMarker (
				__::PhantomData<&'src ()>, __::Infallible
			),
		}
	);
}

/// resolve capture with nested captures: Struct, Tuple, ReduceMap
fn resolve_fielded_capture(
	cap: &mut Capture, resolved_type: &mut TokenStream, create: Create,
	parent: &CapParent, ctx: &mut Context,
) -> Result<CapKind, ()> {
	let is_generated = matches!(create, Create::Struct(_));
	let mut this = CapParent::new(is_generated);
	resolve_captures(&mut cap.expr, false, &mut this, ctx);

	if let Create::Struct(item) = create {
		gen_struct(&item, &this, ctx);
	} else if let Create::Enum(_) = create {
		err!(ctx, "expected root or expression for generated enum", cap.ident.span());
		return Err(());
	}

	if cap.map.is_some() {
		if is_generated {
			err!(ctx, "generated item captures can not have a map", cap.ident.span());
			return Err(());
		}
		Ok(CapKind::ReduceMap(this.children))
	} else if matches!(cap.ty, CapType::Inherited) {
		if parent.is_generated {
			// compute tuple type only when needed, in generated items / Matcher::Capture
			*resolved_type = quote! { ( #for child in &this.children #{
				#do { child.write_type(__stream) },
			})}
		}
		Ok(CapKind::Tuple(this.children))
	} else {
		Ok(CapKind::Struct { fields: this.children, is_generated })
	}
}

/// resolve and validate enum variant
fn resolve_enum_variant(
	expr: &mut Expr, variant_names: &mut FxHashSet<String>,
	variants_def: &mut Option<TokenStream>, ctx: &mut Context,
) -> Option<CapChild> {
	let mut parent = CapParent::new(true);
	match expr {
		Expr::Imply { cond, expr } => {
			resolve_captures(cond, false, &mut parent, ctx);
			// remove unnecessary Option<T>
			resolve_captures(expr, false, &mut parent, ctx);
		}
		_ => resolve_captures(expr, false, &mut parent, ctx),
	}

	if parent.children.len() > 1 {
		for child in &parent.children[1..] {
			let msg = "an or branch in an enum capture must have at most one capture";
			err!(ctx, msg, child.name.span());
		}
	}

	let child = parent.children.pop();
	if let Some(child) = &child {
		if !variant_names.insert(child.name.to_string()) {
			err!(ctx, "a variant exist with the same name", child.name.span());
			return None;
		}

		if let Some(mut def) = variants_def.as_mut() {
			// add variant definition
			chunk!(def, #{pascal_case(&child.name)}(#do { child.write_type(def) }),);
		}
	}
	child
}
fn resolve_enum_variants(
	cap: &mut Capture, is_gen: bool, ctx: &mut Context,
) -> (Vec<Option<CapChild>>, Option<TokenStream>) {
	let mut variants = Vec::new();
	let mut variant_names = FxHashSet::default();
	let mut variants_def = is_gen.then(TokenStream::new);
	let Expr::Or(exprs) = &mut cap.expr else { unreachable!() };
	for expr in exprs {
		let var = resolve_enum_variant(expr, &mut variant_names, &mut variants_def, ctx);
		variants.push(var);
	}
	(variants, variants_def)
}

fn resolve_enum_capture(
	cap: &mut Capture, create: Create, ctx: &mut Context,
) -> Result<CapKind, ()> {
	if cap.map.is_some() {
		err!(ctx, "enum captures can not have a map", cap.ident.span());
		return Err(());
	}

	let (variants, variants_def) =
		resolve_enum_variants(cap, matches!(create, Create::Enum(_)), ctx);

	if let Create::Enum(item) = create {
		gen_enum(&item, variants.iter().any(Option::is_none), variants_def, ctx);
	} else if let Create::Struct(_) = create {
		let msg = "expected root non or expression for generated struct";
		err!(ctx, msg, cap.ident.span());
		return Err(());
	}

	Ok(CapKind::Enum(variants))
}

/// check if capture is atomic from its expression
fn is_atomic_capture(expr: &Expr) -> bool {
	matches!(expr,
		Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom }
		if matches!(atom, Atom::Matcher(_) | Atom::Call { .. })
	)
}

/// resolve captures having no nested captures: Slice, Atomic, UnitStruct
fn resolve_leaf_capture(
	cap: &mut Capture, create: Create, ctx: &mut Context,
) -> Result<CapKind, ()> {
	let need_from = matches!(cap.ty, CapType::Explicit(_)) && cap.map.is_none();
	Ok(match create {
		Create::Struct(item) => {
			gen_unit_struct(&item, ctx);
			CapKind::UnitStruct
		}
		Create::Enum(ident) => {
			err!(ctx, "expected root or expression for generated enum", ident.span());
			return Err(());
		}
		Create::None if is_atomic_capture(&cap.expr) => {
			if let Expr::Unit { atom: Atom::Call { args, .. }, .. } = &mut cap.expr {
				for arg in args {
					analyze_matcher(arg, ctx);
				}
			}
			CapKind::Atomic { need_from }
		}
		Create::None => CapKind::Slice { need_from },
	})
}

/// add capture to a parent
fn add_capture_child(
	cap: &Capture, resolved_type: &TokenStream, container: CapContainer,
	parent: &mut CapParent, ctx: &mut Context,
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

/// try resolving a capture, returning on first error
fn try_resolve_capture(
	cap: &mut Capture, is_optional: bool, parent: &mut CapParent, ctx: &mut Context,
) -> Result<(), ()> {
	let (mut resolved_type, create) = resolve_capture_type(cap, ctx)?;

	let kind = if has_capture(&cap.expr) && !is_atomic_capture(&cap.expr) {
		match &cap.expr {
			Expr::Or(_) => resolve_enum_capture(cap, create, ctx)?,
			_ => resolve_fielded_capture(cap, &mut resolved_type, create, parent, ctx)?,
		}
	} else {
		resolve_leaf_capture(cap, create, ctx)?
	};

	let container = match (is_optional, cap.rep) {
		(false, Rep::ONCE) => CapContainer::None,
		(true, Rep::ONCE) | (_, Rep::OPTIONAL) => CapContainer::Option,
		_ => CapContainer::Vec,
	};

	add_capture_child(cap, &resolved_type, container, parent, ctx)?;

	cap.info = Some(CapInfo { resolved_type, kind, container });

	Ok(())
}
/// resolve a capture atomicly
fn resolve_capture(
	cap: &mut Capture, is_optional: bool, parent: &mut CapParent, ctx: &mut Context,
) -> Result<(), ()> {
	let res = try_resolve_capture(cap, is_optional, parent, ctx);
	if let Err(()) = res {
		// mybe ok nested captures but errored self
		strip_info(&mut cap.expr);
	}
	res
}

/// remove all semantic info from nested captures
fn strip_info(expr: &mut Expr) {
	match expr {
		Expr::And(expr) | Expr::Seq(expr) | Expr::Or(expr) => {
			for expr in expr {
				strip_info(expr);
			}
		}
		Expr::Imply { cond, expr } => {
			strip_info(cond);
			strip_info(expr);
		}
		Expr::Unit { atom: Atom::Group(expr), .. } => strip_info(expr),
		Expr::Capture(cap) => cap.info = None,
		_ => (),
	}
}

/// camel -> pascal
pub fn pascal_case(ident: &Ident) -> Ident {
	let orig = ident.to_string();
	let mut res = String::with_capacity(orig.len());
	for section in orig.split('_') {
		if let Some(char) = section.chars().next() {
			res.push(char.to_ascii_uppercase());
		}
		if let Some(s) = section.get(1..) {
			res.push_str(s);
		}
	}
	Ident::new(&res, ident.span())
}

/// propagate matched type to call atom arguments
fn propagate_matched_type(expr: &mut Expr, matched_type: Option<&TokenStream>) {
	match expr {
		Expr::And(expr) | Expr::Seq(expr) | Expr::Or(expr) => {
			for expr in expr {
				propagate_matched_type(expr, matched_type);
			}
		}
		Expr::Imply { cond, expr } => {
			propagate_matched_type(cond, matched_type);
			propagate_matched_type(expr, matched_type);
		}
		Expr::Capture(cap) => propagate_matched_type(&mut cap.expr, matched_type),
		Expr::Unit { atom: Atom::Group(expr), .. } => {
			propagate_matched_type(expr, matched_type);
		}
		Expr::Unit { atom: Atom::Call { args, .. }, .. } => {
			for arg in args {
				if arg.matched_type.is_none() {
					arg.matched_type = matched_type.cloned();
				}
				propagate_matched_type(&mut arg.cap.expr, arg.matched_type.as_ref());
			}
		}
		_ => {}
	}
}

/// resolve a root capture with all its nested captures
pub fn analyze_root_cap(cap: &mut Capture, concrete_types: bool, ctx: &mut Context) {
	propagate_matched_type(&mut cap.expr, ctx.matched_type);
	let res = resolve_capture(cap, false, &mut CapParent::new(concrete_types), ctx);
	if res.is_err() {
		// root capture get slice kind on errors since matcher impl assume cap.info isnt None
		cap.map = None;
		cap.info = Some(CapInfo {
			resolved_type: default_cap(ctx.matched_type),
			kind: CapKind::Slice { need_from: false },
			container: CapContainer::None,
		});
	}
}

/// resolve all captures inside a matcher
pub fn analyze_matcher(matcher: &mut Matcher, ctx: &mut Context) {
	if matcher.matched_type.is_none() {
		let msg = "expected specified matched type for matchers";
		err!(ctx, msg, Span::call_site());
		return;
	}
	let mut ctx = Context {
		matched_type: matcher.matched_type.as_ref(),
		capture_mod: ctx.capture_mod.as_deref_mut(),
		errors: ctx.errors,
	};

	analyze_root_cap(&mut matcher.cap, true, &mut ctx);
}

/// resolve all captures inside a term
pub fn analyze_term(term: &mut Term, ctx: &mut Context) {
	let Term { args, cap, .. } = term;
	let mut arg_names = FxHashSet::default();
	for arg in args {
		if !arg_names.insert(arg.to_string()) {
			err!(ctx, "another argument exist with the same name", arg.span());
			*arg = Ident::new("_", arg.span());
		}
	}

	analyze_root_cap(cap, true, ctx);
}
