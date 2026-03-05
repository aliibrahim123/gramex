use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::{Error, Ident};

use crate::{
	gen_types,
	parse::{self, Atom, Expr, GramexMacro, Term},
};

fn default<T: Default>() -> T {
	T::default()
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Repetition {
	Once,
	Optional,
	Multiple,
}
#[derive(Debug, Clone)]
enum CaptureNodeKind<'a> {
	Normal,
	Group(Vec<CaptureNode<'a>>),
	Term(TokenStream),
	Enum { with_none: bool, nodes: Vec<CaptureNode<'a>> },
}
#[derive(Debug, Clone)]
struct CaptureNode<'a> {
	expr: &'a Expr,
	kind: CaptureNodeKind<'a>,
	rep: Repetition,
}
/// a simplified expression tree featuring only captures
#[derive(Debug, Clone)]
enum CapTree<'a> {
	Capture(CaptureNode<'a>),
	Enum { with_none: bool, nodes: Vec<CaptureNode<'a>> },
	Group(Vec<CaptureNode<'a>>),
}

fn test_capture_term<'a>(expr: &Expr, terms: &'a Vec<Term>) -> CaptureNodeKind<'a> {
	let Expr::Unit { atom: Atom::Term(path), .. } = expr else { return CaptureNodeKind::Normal };
	let Some(ident) = path.first() else { return CaptureNodeKind::Normal };
	let Some(term) = terms.iter().find(|t| t.name == *ident) else {
		return CaptureNodeKind::Normal;
	};
	if path.len() == 1 {
		let mod_name = format_ident!("{}_captures", term.name);
		CaptureNodeKind::Term(quote! {super::#mod_name::Root<'a>})
	} else {
		CaptureNodeKind::Normal
	}
}
fn map_child<'a>(
	expr: &'a Vec<Expr>, terms: &'a Vec<Term>,
) -> syn::Result<Vec<gen_types::CaptureNode<'a>>> {
	let mut children = vec![];
	for expr in expr {
		match transform(expr, terms)? {
			Some(CapTree::Group(nodes)) => children.extend(nodes),
			Some(CapTree::Capture(node)) => children.push(node),
			_ => continue,
		}
	}
	Ok(children)
}
fn transform<'a>(expr: &'a Expr, terms: &'a Vec<Term>) -> syn::Result<Option<CapTree<'a>>> {
	use CapTree::*;
	Ok(match expr {
		Expr::And(exprs) | Expr::Seq(exprs) => {
			let mut nodes = map_child(exprs, terms)?;
			match nodes.len() {
				0 => None,
				1 => Some(Capture(nodes.remove(0))),
				_ => Some(Group(nodes)),
			}
		}
		Expr::Or(exprs) => {
			let nodes = map_child(exprs, terms)?;
			match nodes.is_empty() {
				true => None,
				false => Some(Enum { with_none: exprs.len() != nodes.len(), nodes }),
			}
		}
		cap @ Expr::Capture { rep, expr, .. } => {
			let nodes = transform(expr, terms)?;
			let rep = match *rep {
				parse::Repetition::Once => Repetition::Once,
				parse::Repetition::Optional => Repetition::Optional,
				_ => Repetition::Multiple,
			};
			type Kind<'a> = CaptureNodeKind<'a>;
			let kind = match nodes {
				Some(Group(nodes)) => Kind::Group(nodes),
				Some(Enum { with_none, nodes }) => Kind::Enum { with_none, nodes },
				Some(Capture(node)) => Kind::Group(vec![node]),
				None => test_capture_term(expr, terms),
			};
			Some(Capture(gen_types::CaptureNode { expr: cap, rep, kind }))
		}
		Expr::Unit { not, near, repetition, atom, .. } => {
			if near | not {
				find_unallowed_capture(expr)?;
			};
			if *repetition != parse::Repetition::Once {
				find_unallowed_capture(expr)?;
			}
			match atom {
				Atom::Group(expr) => transform(expr, terms)?,
				Atom::Call { args, .. } => {
					for arg in args {
						find_unallowed_capture(arg)?
					}
					None
				}
				_ => None,
			}
		}
		_ => None,
	})
}
fn find_unallowed_capture(expr: &Expr) -> syn::Result<()> {
	match expr {
		Expr::Capture { ident, .. } => Err(Error::new(ident.span(), "unallowed capture"))?,
		Expr::And(exprs) | Expr::Or(exprs) | Expr::Seq(exprs) => {
			for expr in exprs {
				find_unallowed_capture(expr)?
			}
		}
		Expr::Unit { atom, .. } => match atom {
			Atom::Group(expr) => find_unallowed_capture(expr)?,
			Atom::Call { args, .. } => {
				for arg in args {
					find_unallowed_capture(arg)?
				}
			}
			_ => (),
		},
		_ => (),
	};
	Ok(())
}

#[derive(Debug, Clone, Default)]
pub enum CaptureKind {
	#[default]
	Normal,
	Term(TokenStream),
	Group(Vec<Ident>),
	Enum {
		with_none: bool,
	},
}
#[derive(Debug, Clone, Default)]
pub struct CaptureInfo {
	pub type_name: Option<Ident>,
	pub kind: CaptureKind,
	pub enum_type: Option<Ident>,
}
struct ResolveResult {
	pub resolved: TokenStream,
	pub has_map: bool,
	pub cap: CaptureInfo,
}

fn write_rep(ty: impl ToTokens, rep: Repetition) -> TokenStream {
	match rep {
		Repetition::Once => quote! {#ty},
		Repetition::Optional => quote! {Option<#ty>},
		Repetition::Multiple => quote! {Vec<#ty>},
	}
}
fn resolve_capture(
	node: &CaptureNode, matched_type: &TokenStream, cap_defs: &mut TokenStream,
	id_counter: &mut u64,
) -> ResolveResult {
	let CaptureNode { expr, kind, rep } = node;
	let Expr::Capture { ty, type_info, .. } = expr else { unreachable!() };

	use CaptureNodeKind::*;
	if matches!(kind, Normal | Term(_)) {
		let resolved = if let Some(ty) = ty {
			write_rep(ty.as_ref(), *rep)
		} else {
			if let Term(res) = kind { write_rep(res, *rep) } else { write_rep(matched_type, *rep) }
		};
		let kind =
			if let Term(res) = kind { CaptureKind::Term(res.clone()) } else { CaptureKind::Normal };
		return ResolveResult { resolved, has_map: false, cap: CaptureInfo { kind, ..default() } };
	}

	let id = *id_counter;
	*id_counter += 1;
	let name = format_ident!("Cap{id}");
	let resolved = if let Some(ty) = ty {
		write_rep(ty.as_ref(), *rep)
	} else {
		write_rep(quote! {#name<'a>}, *rep)
	};

	let mut map_def = TokenStream::new();
	let mut loop_children =
		|nodes, looper: &mut dyn FnMut(&syn::Ident, TokenStream, &mut CaptureInfo)| {
			for node in nodes {
				let ResolveResult { resolved, mut cap, has_map } =
					resolve_capture(node, matched_type, cap_defs, id_counter);
				let Expr::Capture { ident, type_info, .. } = node.expr else { unreachable!() };
				looper(ident, resolved, &mut cap);
				if let Some(type_name) = &cap.type_name {
					let types = format_ident!("{ident}_types");
					let types_mod = format_ident!("{type_name}_types");
					map_def.append_all(quote! { pub type #ident<'a> = #type_name<'a>; });
					if has_map {
						map_def.append_all(quote! {pub use super::#types_mod as #types;});
					}
				}
				type_info.replace(cap);
			}
		};

	let kind = match kind {
		Group(nodes) => {
			let mut fields = vec![];
			let mut fields_def = TokenStream::new();
			loop_children(nodes, &mut |ident, resolved, _| {
				fields.push(ident.clone());
				fields_def.append_all(quote! {pub #ident: #resolved,});
			});
			cap_defs.append_all(quote! {#[derive(Debug)] pub struct #name<'a> {
				pub matched: #matched_type, #fields_def
				#[doc(hidden)] pub __life_marker: std::marker::PhantomData<&'a ()>
			}});
			CaptureKind::Group(fields)
		}
		Enum { with_none, nodes } => {
			let mut var_def = TokenStream::new();
			loop_children(nodes, &mut |ident, resolved, cap| {
				var_def.append_all(quote! {#ident(#resolved),});
				cap.enum_type = Some(name.clone());
			});
			let none = with_none.then(|| quote! {None,});
			cap_defs.append_all(quote! { #[derive(Debug)] pub enum #name<'a> {
				#none #var_def
				#[doc(hidden)] __life_marker(std::convert::Infallible, std::marker::PhantomData<&'a ()>)
			} });
			CaptureKind::Enum { with_none: *with_none }
		}
		_ => unreachable!(),
	};
	if !map_def.is_empty() {
		let name = format_ident!("Cap{id}_types");
		cap_defs.append_all(quote! {pub mod #name {use super::*; #map_def}});
	};
	let cap = CaptureInfo { kind, type_name: Some(name), ..default() };
	ResolveResult { resolved, cap, has_map: !map_def.is_empty() }
}

pub struct ResolveExprResult {
	pub mod_def: TokenStream,
	pub root_type: TokenStream,
}
fn resolve_types(
	expr: &Expr, mod_name: &Ident, matched_type: &syn::Type, id_counter: &mut u64,
	terms: &Vec<Term>,
) -> syn::Result<ResolveExprResult> {
	let Some(CapTree::Capture(root)) = transform(&expr, &terms)? else { unreachable!() };
	let Expr::Capture { type_info, .. } = root.expr else { unreachable!() };
	let mut type_defs = TokenStream::new();
	let matched_type = quote! { &'a #matched_type };

	let ResolveResult { cap, resolved, has_map, .. } =
		resolve_capture(&root, &matched_type, &mut type_defs, id_counter);

	let Some(type_name) = &cap.type_name else {
		return Ok(ResolveExprResult { mod_def: TokenStream::new(), root_type: resolved });
	};

	type_defs.append_all(quote! {pub type Root<'a> = #type_name<'a>;});
	if has_map {
		let types = format_ident!("{}_types", type_name);
		type_defs.append_all(quote! {pub use #types as root_types;});
	}
	let mod_def = quote! {pub mod #mod_name { use super::*; #type_defs }};
	let resolved = quote! { #mod_name::#type_name<'a> };
	type_info.replace(cap);
	Ok(ResolveExprResult { mod_def, root_type: resolved })
}

pub fn resolve_types_expr(
	expr: &Expr, matched_type: &syn::Type, mod_name: &Ident,
) -> syn::Result<TokenStream> {
	let mut id_counter = 0;
	let ResolveExprResult { mod_def, .. } =
		resolve_types(expr, mod_name, matched_type, &mut id_counter, &vec![])?;
	Ok(mod_def)
}
pub fn resolve_types_macro(modu: &mut GramexMacro) -> syn::Result<TokenStream> {
	let GramexMacro { matched_type, .. } = modu;
	let mut mod_defs = TokenStream::new();
	let mut resolved_types = vec![];
	let mut id_counter = 1;

	for term in &modu.terms {
		let name = &format_ident!("{}_captures", term.name);
		let ResolveExprResult { mod_def, root_type } =
			resolve_types(&term.expr, name, matched_type, &mut id_counter, &modu.terms)?;
		resolved_types.push(root_type);
		mod_defs.append_all(mod_def);
	}
	for (ind, ty) in resolved_types.into_iter().enumerate() {
		modu.terms[ind].resolved = ty;
	}

	Ok(mod_defs)
}
