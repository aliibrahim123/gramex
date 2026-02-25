use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::{Error, Ident};

use crate::{
	gen_types,
	parse::{self, Atom, Expr, GramexMacro, Term},
};

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
	Enum(Vec<CaptureNode<'a>>),
	EnumWithNone(Vec<CaptureNode<'a>>),
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
	Enum { has_none: bool, nodes: Vec<CaptureNode<'a>> },
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
				false => Some(Enum { has_none: exprs.len() != nodes.len(), nodes }),
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
			let ty = match nodes {
				Some(Group(nodes)) => Kind::Group(nodes),
				Some(Enum { has_none: false, nodes }) => Kind::Enum(nodes),
				Some(Enum { has_none: true, nodes }) => Kind::EnumWithNone(nodes),
				Some(Capture(node)) => Kind::Group(vec![node]),
				None => test_capture_term(expr, terms),
			};
			Some(Capture(gen_types::CaptureNode { expr: cap, rep, kind: ty }))
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

#[derive(Debug)]
pub enum CaptureKind {
	Group,
	Term,
	Enum,
	EnumWithNone,
}
#[derive(Debug)]
pub struct Type {
	pub id: u32,
	pub name: Ident,
	pub kind: CaptureKind,
	pub enum_type: Option<String>,
	pub fields: Vec<String>,
}
#[derive(Default)]
struct ResolveResult {
	pub resolved: TokenStream,
	pub name: Option<Ident>,
	pub has_map: bool,
	pub ty: Option<Type>,
}

fn write_rep(ty: impl ToTokens, rep: Repetition) -> TokenStream {
	match rep {
		Repetition::Once => quote! {#ty},
		Repetition::Optional => quote! {Option<#ty>},
		Repetition::Multiple => quote! {Vec<#ty>},
	}
}
fn resolve_capture(
	node: &CaptureNode, matched_type: &syn::Type, type_defs: &mut TokenStream,
	id_counter: &mut u32, types: &mut HashMap<u32, Type>,
) -> ResolveResult {
	let CaptureNode { expr, kind, rep } = node;
	let Expr::Capture { ty, typeid, .. } = expr else { unreachable!() };

	use CaptureNodeKind::*;
	if matches!(kind, Normal | Term(_)) {
		let resolved = if let Some(ty) = ty {
			write_rep(ty.as_ref(), *rep)
		} else {
			if let Term(res) = kind { write_rep(res, *rep) } else { write_rep(matched_type, *rep) }
		};
		return ResolveResult { resolved, ..Default::default() };
	}

	let id = *id_counter;
	*id_counter += 1;
	typeid.replace(id);
	let name = format_ident!("Cap{id}");
	let resolved = if let Some(ty) = ty {
		write_rep(ty.as_ref(), *rep)
	} else {
		write_rep(quote! {#name<'a>}, *rep)
	};

	let mut map_def = TokenStream::new();
	let mut loop_children =
		|nodes, looper: &mut dyn FnMut(&syn::Ident, TokenStream, &mut Option<Type>)| {
			for node in nodes {
				let ResolveResult { resolved, name, mut ty, has_map } =
					resolve_capture(node, matched_type, type_defs, id_counter, types);
				let Expr::Capture { ident, .. } = node.expr else { unreachable!() };
				looper(ident, resolved, &mut ty);
				if let Some(ty) = ty {
					types.insert(ty.id, ty);
				}
				if let Some(resolved) = name {
					let types = format_ident!("{ident}_types");
					let types_mod = format_ident!("{resolved}_types");
					map_def.append_all(quote! { pub type #ident<'a> = #resolved<'a>; });
					if has_map {
						map_def.append_all(quote! {pub use super::#types_mod as #types;});
					}
				}
			}
		};

	let (kind, fields) = match kind {
		Group(nodes) => {
			let mut fields = vec![];
			let mut fields_def = TokenStream::new();
			loop_children(nodes, &mut |ident, resolved, _| {
				fields.push(ident.to_string());
				fields_def.append_all(quote! {pub #ident: #resolved,});
			});
			type_defs.append_all(quote! {pub struct #name<'a> {
				pub matched: #matched_type, #fields_def
				#[doc(hidden)]
				pub __life_marker: std::marker::PhantomData<&'a ()>
			}});
			(CaptureKind::Group, fields)
		}
		Enum(nodes) | EnumWithNone(nodes) => {
			let with_none = matches!(kind, EnumWithNone(_));
			let mut var_def = TokenStream::new();
			loop_children(nodes, &mut |ident, resolved, ty| {
				var_def.append_all(quote! {#ident(#resolved),});
				if let Some(ty) = ty {
					ty.enum_type = Some(name.to_string());
				};
			});
			let none = with_none.then(|| quote! {None,});
			type_defs.append_all(quote! {pub enum #name<'a> {#none #var_def}});
			let kind = if with_none { CaptureKind::EnumWithNone } else { CaptureKind::Enum };
			(kind, vec![])
		}
		_ => unreachable!(),
	};
	if !map_def.is_empty() {
		let name = format_ident!("Cap{id}_types");
		type_defs.append_all(quote! {pub mod #name {use super::*; #map_def}});
	};
	let ty = Type { kind, id, name: name.clone(), fields, enum_type: None };
	ResolveResult { resolved, ty: Some(ty), name: Some(name), has_map: !map_def.is_empty() }
}

pub struct ResolveExprResult {
	pub mod_def: TokenStream,
	pub root_type: TokenStream,
}
fn resolve_types(
	expr: &Expr, mod_name: &Ident, matched_type: &syn::Type, id_counter: &mut u32,
	types: &mut HashMap<u32, Type>, terms: &Vec<Term>,
) -> syn::Result<ResolveExprResult> {
	let Some(CapTree::Capture(root)) = transform(&expr, &terms)? else { unreachable!() };
	let mut type_defs = TokenStream::new();

	let ResolveResult { ty, resolved, name, has_map, .. } =
		resolve_capture(&root, &matched_type, &mut type_defs, id_counter, types);

	let Some(ty) = ty else {
		return Ok(ResolveExprResult { mod_def: TokenStream::new(), root_type: resolved });
	};
	types.insert(ty.id, ty);

	type_defs.append_all(quote! {pub type Root<'a> = #name<'a>;});
	if has_map {
		let types = format_ident!("{}_types", name.unwrap());
		type_defs.append_all(quote! {pub use #types as root_types;});
	}
	let mod_def = quote! {pub mod #mod_name {#type_defs}};
	Ok(ResolveExprResult { mod_def, root_type: resolved })
}

pub fn resolve_types_expr(
	expr: &Expr, matched_type: &syn::Type, mod_name: &Ident,
) -> syn::Result<TokenStream> {
	let mut types = HashMap::new();
	let mut id_counter = 0;
	let ResolveExprResult { mod_def, .. } =
		resolve_types(expr, mod_name, matched_type, &mut id_counter, &mut types, &vec![])?;
	Ok(mod_def)
}
pub fn resolve_types_macro(modu: &mut GramexMacro) -> syn::Result<TokenStream> {
	let GramexMacro { matched_type, .. } = modu;
	let mut mod_defs = TokenStream::new();
	let mut types = HashMap::new();
	let mut resolved_types = vec![];
	let mut id_counter = 0;

	for term in &modu.terms {
		let name = &format_ident!("{}_captures", term.name);
		let terms = &modu.terms;
		let ResolveExprResult { mod_def, root_type } =
			resolve_types(&term.expr, name, matched_type, &mut id_counter, &mut types, terms)?;
		resolved_types.push(root_type);
		mod_defs.append_all(mod_def);
	}
	for (ind, ty) in resolved_types.into_iter().enumerate() {
		modu.terms[ind].resolved = ty;
	}

	Ok(mod_defs)
}
