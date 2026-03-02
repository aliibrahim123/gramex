use std::{collections::HashMap, fmt::format};

use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::Ident;

use crate::{
	gen_types::{CaptureInfo, CaptureKind},
	parse::{Atom, Expr, Repetition, Term},
};

pub struct Ctx<'a> {
	pub captures_mod: &'a Ident,
}

fn gen_atom(atom: &Atom, ctx: &Ctx) -> TokenStream {
	fn match_by(t: impl ToTokens) -> TokenStream {
		quote! { gramex::MatchAble::match_by(value, #t, ind, status) }
	}
	match atom {
		Atom::Literal(lit) => match_by(lit),
		Atom::Term(path) => match_by(path),
		Atom::Block(block) => match_by(block),
		Atom::Any => quote! {{ *ind += 1; gramex::MatchSignal::Matched }},
		Atom::Group(expr) => gen_expr(expr, ctx),
		Atom::Call { path, args } => {
			let mut args_res = quote! {};
			for arg in args {
				let mat = gen_expr(arg, ctx);
				args_res.append_all(quote! { |value, ind, status| #mat, });
			}
			quote! { #path(value, #args_res, ind, status) }
		}
	}
}

fn gen_rep(repetition: &Repetition, matcher: TokenStream) -> TokenStream {
	if *repetition == Repetition::Once {
		return matcher;
	} else if *repetition == Repetition::Optional {
		return quote! { 'mat: {
			let start_ind = *ind;
			let status = gramex::MatchStatus { in_main_path: false, ..status };
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { *ind = start_ind };
			gramex
		}};
	};

	let Repetition(start, end) = *repetition;
	let mut match_block = quote! { start_ind = *ind; };
	if start == 0 {
		match_block.append_all(quote! {
			let state = &gramex::MatchStatus { in_main_path: false, ..status };
		});
	} else {
		match_block.append_all(quote! {
			let state = if iter < #start { state } else {
				&gramex::MatchStatus { in_main_path: false, ..status };
			}
		});
	}

	match_block.append_all(quote! {
		let sig = #matcher;
		let matched = sig == gramex::MatchSignal::Matched;
	});

	if end != u32::MAX {
		match_block.append_all(quote! {
			if iter >= #end {
				*ind = start_ind;
				if matched { break 'mat gramex::MatchSignal::MisMatched }
				else { break 'mat gramex::MatchSignal::Matched }
			};
		});
	}

	let mut mismatch_block = quote! {};
	if start != 0 {
		mismatch_block.append_all(quote! {
			if iter < #start { break 'mat gramex::MatchSignal::MisMatched };
		});
	}
	mismatch_block.append_all(quote! {
		*ind = start_ind;
		break 'mat gramex::MatchSignal::Matched
	});
	match_block.append_all(quote! { if !matched { #mismatch_block } });

	quote! { 'mat: for iter in .. { #match_block } }
}

fn gen_unit(unit: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Unit { not, near, repetition, atom } = unit else { unreachable!() };
	fn gen_forked_match(matcher: TokenStream, mapper: TokenStream) -> TokenStream {
		quote! { 'mat: {
			let mut ind = &mut *ind;
			let status = &gramex::MatchStatus { in_main_path: false, ..status };
			let sig = #matcher,
			break 'mat #mapper
		} }
	}
	let matcher = gen_atom(atom, ctx);
	if *near {
		let mapper = if *not {
			quote! { match sig != gramex::MatchSignal::Matched {
				true => gramex::MatchSignal::Matched,
				false => gramex::MatchSignal::MisMatched
			} }
		} else {
			quote! { match sig {
				gramex::MatchSignal::Matched => gramex::MatchSignal::Matched,
				err => err
			} }
		};
		gen_forked_match(gen_rep(repetition, matcher), mapper)
	} else if *not {
		let mapper = quote! { match sig != gramex::MatchSignal::Matched {
			true => { *ind += 1; gramex::MatchSignal::Matched },
			false => gramex::MatchSignal::MisMatched
		} };
		gen_rep(repetition, gen_forked_match(matcher, mapper))
	} else {
		gen_rep(repetition, matcher)
	}
}

fn gen_range(range: &Expr) -> TokenStream {
	let Expr::Range(lh, rh) = range else { unreachable!() };
	fn gen_atom(atom: &Atom) -> TokenStream {
		match atom {
			Atom::Term(term) => quote! { #term },
			Atom::Literal(lit) => quote! { #lit },
			Atom::Block(block) => quote! { #block },
			Atom::Group(expr) => match &**expr {
				Expr::Unit { atom, .. } => gen_atom(atom),
				_ => unreachable!(),
			},
			_ => unreachable!(),
		}
	}
	let (lh, rh) = (gen_atom(lh), gen_atom(rh));
	quote! { gramex::MatchAble::match_by(value, #lh..#rh, ind, status) }
}

fn gen_seq(seq: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Seq(exprs) = seq else { unreachable!() };
	let mut match_block = quote! {};
	for expr in exprs {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	quote! { 'mat: { #match_block; gramex::MatchSignal::Matched } }
}

fn gen_or(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Or(exprs) = expr else { unreachable!() };
	let mut match_block = quote! {
		let start_ind = *ind;
		let start_status = status;
		let status = &gramex::MatchStatus { in_main_path: false, ..start_status };
	};
	for expr in &exprs[0..exprs.len() - 1] {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig == gramex::MatchSignal::Matched { break 'mat sig }
			*ind = start_ind;
		});
	}
	let last_matcher = gen_expr(exprs.last().unwrap(), ctx);
	match_block.append_all(quote! { status = start_status; #last_matcher });
	quote! { 'mat: { #match_block } }
}

fn gen_and(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::And(exprs) = expr else { unreachable!() };
	let primary_matcher = gen_expr(&exprs[0], ctx);
	let mut match_block = quote! {
		let start_ind = *ind;
		let sig = #primary_matcher;
		if sig != gramex::MatchSignal::Matched { break 'mat sig }
		let value = gramex::MatchAble::slice(value, 0..*ind);
	};
	for expr in &exprs[1..] {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let ind = &mut start_ind.clone();
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	quote! { 'mat: { #match_block; gramex::MatchSignal::Matched } }
}

fn gen_capture(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let Ctx { captures_mod } = ctx;
	let Expr::Capture { ident, rep, ty, conv, type_info, expr } = expr else { unreachable!() };
	let type_info = type_info.borrow();
	let CaptureInfo { type_name, kind, enum_type } = &*type_info;

	let matcher = gen_expr(expr, ctx);
	let matcher = quote! {
		let start_ind = *ind;
		let sig = #matcher;
		if sig != gramex::MatchSignal::Matched { break 'mat sig }
	};
	let mut match_block = quote! {};

	match kind {
		CaptureKind::Normal => match_block.append_all(quote! {
			#matcher
			let cap = gramex::MatchAble::slice(value, start_ind..*ind);
		}),

		CaptureKind::Term(term) => match_block.append_all(quote! {
			let Some(cap) = #term(value, ind, status)
				else { break 'mat gramex::MatchSignal::MisMatched };
		}),
		CaptureKind::Group(fields) => {
			let mut struct_init = quote! {};
			for field in fields {
				let name = format_ident!("cap_{field}");
				match_block.append_all(quote! { let #name; });
				struct_init.append_all(quote! { #field: #name, });
			}
			match_block.append_all(quote! {
				#matcher
				let matched = gramex::MatchAble::slice(value, start_ind..*ind);
				let cap = #captures_mod::#type_name { matched, #struct_init };
			});
		}
		CaptureKind::Enum { with_none } => {
			if *with_none {
				match_block = quote! {
					let cap_enum = #captures_mod::#type_name::None;
					#matcher; let cap = cap_enum;
				};
			} else {
				match_block.append_all(quote! { let cap_enum; #matcher });
			}
		}
	};

	if let Some(conv) = conv {
		match_block.append_all(quote! { let cap = (#conv)(cap); });
	} else if let Some(ty) = ty {
		match_block.append_all(quote! { let cap = #ty::from(cap); });
	}

	let captured = format_ident!("{}", if *rep == Repetition::Once { "cap" } else { "captured" });
	if *rep != Repetition::Once {
		let (add, initial) = match *rep {
			Repetition::Optional => (quote! { = Some}, quote! { None }),
			_ => (quote! { .push }, quote! { Vec::new() }),
		};
		let matcher = quote! { 'mat: {
			#matcher;
			captured #add(cap);
		}};
		let matcher = gen_rep(&rep, matcher);
		match_block = quote! {
			let captured = #initial;
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { break 'mat sig }
		};
	}

	let name = format_ident!("cap_{ident}");
	if let Some(enum_name) = enum_type {
		match_block.append_all(quote! { cap_enum = #captures_mod::#enum_name::#ident(#captured); });
	} else {
		match_block.append_all(quote! { #name = #captured; });
	}
	quote! { 'mat: { #match_block gramex::MatchSignal::Matched } }
}

pub fn gen_expr(expr: &Expr, ctx: &Ctx) -> TokenStream {
	match expr {
		Expr::Unit { .. } => gen_unit(expr, ctx),
		Expr::Range(_, _) => gen_range(expr),
		Expr::Seq(_) => gen_seq(expr, ctx),
		Expr::Or(_) => gen_or(expr, ctx),
		Expr::And(_) => gen_and(expr, ctx),
		Expr::Capture { .. } => gen_capture(expr, ctx),
	}
}

pub fn gen_matcher(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let root_matcher = gen_capture(expr, ctx);
	quote! {
		let cap_root;
		let sig = #root_matcher;
		if sig != gramex::MatchSignal::Matched { return Err(sig.into_err(*ind)) }
	}
}
pub fn gen_term(term: &Term, match_target: &syn::Type, ctx: &Ctx) -> TokenStream {
	let Term { name, args, resolved, expr } = term;
	let mut args_tokens = quote! {};
	for arg in args {
		args_tokens.append_all(quote! { #arg: impl gramex::Matcher<#match_target>, });
	}

	let matcher = gen_expr(expr, ctx);
	let match_name = format_ident!("match_{name}");
	quote! {
		pub fn #name (
			value: #match_target, #args_tokens, ind: &mut usize, status: gramex::MatchStatus
		) -> gramex::MatchResult<#resolved> {
			#matcher;
			Ok(cap_root)
		}

		pub fn #match_name (value: #match_target, #args_tokens) -> Option<#resolved> {
			let mut ind = &mut 0;
			let status = &gramex::MatchStatus::default();
			#matcher
			if *ind != gramex::MatchAble::len(value) {
				return Err(gramex::MatchError::excess(*ind))
			}
			Ok(cap_root)
		}
	}
}
