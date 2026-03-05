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
		quote! { <_ as ::gramex::MatchBy::<_>>::match_by(value, #t, ind, status) }
	}
	match atom {
		Atom::Literal(lit) => match_by(lit),
		Atom::Term(path) => match_by(path),
		Atom::Block(block) => match_by(block),
		Atom::Any => quote! {{<_ as ::gramex::MatchAble>::skip_1(value, ind, status)}},
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
		return quote! { {
			let start_ind = *ind;
			let was_in_main_path = status.in_main_path;
			status.in_main_path = false;
			let sig = #matcher;
			status.in_main_path = was_in_main_path;
			if sig != ::gramex::MatchSignal::Matched { *ind = start_ind };
			::gramex::MatchSignal::Matched
		}};
	};

	let Repetition(start, end) = *repetition;
	let mut match_block = quote! {
		let start_ind = *ind;
		let was_in_main_path = status.in_main_path;
	};
	if start == 0 {
		match_block.append_all(quote! {	status.in_main_path = false; });
	} else {
		match_block.append_all(quote! {
			if iter >= #start { status.in_main_path = false; }
		});
	}
	match_block.append_all(quote! {
		let sig = #matcher; status.in_main_path = was_in_main_path;
	});

	let mut mismatch_block = quote! {};
	if start != 0 {
		mismatch_block.append_all(quote! {
			if iter < #start { break 'mat ::gramex::MatchSignal::MisMatched };
		});
	}
	if start != end {
		mismatch_block.append_all(quote! {
			*ind = start_ind;
			break 'mat ::gramex::MatchSignal::Matched
		});
	}
	match_block.append_all(quote! { if sig != ::gramex::MatchSignal::Matched { #mismatch_block } });

	match_block.append_all(quote! { iter += 1; });
	if end != u32::MAX {
		match_block.append_all(quote! {
			if iter == #end { break 'mat ::gramex::MatchSignal::Matched };
		})
	}
	quote! { {
		let mut iter = 0;
		'mat: loop { #match_block }
	} }
}

fn gen_unit(unit: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Unit { not, near, repetition, atom } = unit else { unreachable!() };
	fn gen_forked_match(matcher: TokenStream, mapper: TokenStream) -> TokenStream {
		quote! { {
			let real_ind = &mut *ind;
			let ind = &mut real_ind.clone();
			let was_in_main_path = status.in_main_path;
			status.in_main_path = false;
			let sig = #matcher;
			status.in_main_path = was_in_main_path;
			#mapper
		} }
	}
	let matcher = gen_atom(atom, ctx);
	if *near {
		let mapper = if *not {
			quote! { match sig != ::gramex::MatchSignal::Matched {
				true => ::gramex::MatchSignal::Matched,
				false => ::gramex::MatchSignal::MisMatched,
			} }
		} else {
			quote! { sig }
		};
		gen_forked_match(gen_rep(repetition, matcher), mapper)
	} else if *not {
		let mapper = quote! { match sig {
			::gramex::MatchSignal::InComplete => sig,
			::gramex::MatchSignal::Matched => ::gramex::MatchSignal::MisMatched,
			_ => { *real_ind += 1; ::gramex::MatchSignal::Matched },
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
	quote! { <_ as ::gramex::MatchBy::<_>>::match_by(value, #lh..=#rh, ind, status) }
}

fn gen_seq(seq: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Seq(exprs) = seq else { unreachable!() };
	let mut match_block = quote! {};
	for expr in &exprs[0..exprs.len() - 1] {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig != ::gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	match_block.append_all(gen_expr(exprs.last().unwrap(), ctx));
	quote! { 'mat: { #match_block  } }
}

fn gen_or(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::Or(exprs) = expr else { unreachable!() };
	let mut match_block = quote! {
		let start_ind = *ind;
		let was_in_main_path = status.in_main_path;
		status.in_main_path = false;
	};
	for expr in &exprs[0..exprs.len() - 1] {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig == ::gramex::MatchSignal::Matched { break 'mat sig }
			*ind = start_ind;
		});
	}
	let last_matcher = gen_expr(exprs.last().unwrap(), ctx);
	match_block.append_all(quote! { status.in_main_path = was_in_main_path; #last_matcher });
	quote! { 'mat: { #match_block } }
}

fn gen_and(expr: &Expr, ctx: &Ctx) -> TokenStream {
	let Expr::And(exprs) = expr else { unreachable!() };
	let primary_matcher = gen_expr(&exprs[0], ctx);
	let mut match_block = quote! {
		let start_ind = *ind;
		let sig = #primary_matcher;
		if sig != ::gramex::MatchSignal::Matched { break 'mat sig }
		let value = <_ as ::gramex::MatchAble>::slice(value, 0..*ind);
	};
	for expr in &exprs[1..] {
		let matcher = gen_expr(expr, ctx);
		match_block.append_all(quote! {
			let ind = &mut start_ind.clone();
			let sig = #matcher;
			if sig != ::gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	quote! { 'mat: { #match_block; ::gramex::MatchSignal::Matched } }
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
		if sig != ::gramex::MatchSignal::Matched { break 'mat sig }
	};
	let mut match_block = quote! {};

	match kind {
		CaptureKind::Normal => match_block.append_all(quote! {
			#matcher
			let cap = <_ as ::gramex::MatchAble>::slice(value, start_ind..*ind);
		}),

		CaptureKind::Term(term) => match_block.append_all(quote! {
			let Some(cap) = #term(value, ind, status)
				else { break 'mat ::gramex::MatchSignal::MisMatched };
		}),
		CaptureKind::Group(fields) => {
			let mut struct_init = quote! {};
			for field in fields {
				let name = format_ident!("cap_{field}");
				match_block.append_all(quote! { let mut #name = None; });
				struct_init.append_all(quote! { #field: unsafe { #name.unwrap_unchecked() }, });
			}
			match_block.append_all(quote! {
				#matcher
				let matched = <_ as ::gramex::MatchAble>::slice(value, start_ind..*ind);
				let cap = #captures_mod::#type_name {
					matched, #struct_init __life_marker: std::marker::PhantomData
				};
			});
		}
		CaptureKind::Enum { with_none } => {
			let initial = match *with_none {
				true => quote! { Some(#captures_mod::#type_name::None) },
				false => quote! { None },
			};
			match_block.append_all(quote! {
				let mut cap_enum = #initial;
				#matcher
				let cap = unsafe { cap_enum.unwrap_unchecked() };
			});
		}
	};

	if let Some(conv) = conv {
		match_block.append_all(quote! { let cap = ::gramex::__private::conv(cap, #conv); });
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
			#match_block;
			captured #add(cap);
			::gramex::MatchSignal::Matched
		}};
		let matcher = gen_rep(&rep, matcher);
		match_block = quote! {
			let mut captured = #initial;
			let sig = #matcher;
			if sig != ::gramex::MatchSignal::Matched { break 'mat sig }
		};
	}

	let name = format_ident!("cap_{ident}");
	if let Some(enum_name) = enum_type {
		match_block
			.append_all(quote! { cap_enum = Some(#captures_mod::#enum_name::#ident(#captured)); });
	} else {
		match_block.append_all(quote! { #name = Some(#captured); });
	}
	quote! { 'mat: { #match_block ::gramex::MatchSignal::Matched } }
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
		let status = &mut status.clone();
		let mut cap_root = None;
		let sig = #root_matcher;
		if sig != ::gramex::MatchSignal::Matched { return Err(sig.into_err(*ind)) }
	}
}
pub fn gen_term(term: &Term, match_target: &syn::Type, ctx: &Ctx) -> TokenStream {
	let Term { name, args, resolved, expr } = term;
	let mut args_tokens = quote! {};
	for arg in args {
		args_tokens.append_all(quote! { #arg: impl ::gramex::Matcher<#match_target>, });
	}
	let mut args_list = quote! {};
	args_list.append_terminated(args, quote! { , });

	let matcher = gen_matcher(expr, ctx);
	let match_name = format_ident!("match_{name}");
	quote! {
		pub fn #name<'a> (
			value: &'a #match_target, #args_tokens ind: &mut usize, status: &::gramex::MatchStatus
		) -> ::gramex::MatchResult<#resolved> {
			#matcher;
			Ok(unsafe { cap_root.unwrap_unchecked() })
		}

		pub fn #match_name<'a> (value: &'a #match_target, #args_tokens)
			-> ::gramex::MatchResult<#resolved>
		{
			let ind = &mut 0;
			let status = &::gramex::MatchStatus::default();
			let cap = #name(value, #args_list ind, status)?;
			if *ind != <_ as ::gramex::MatchAble>::len(value) {
				return Err(::gramex::MatchError::excess(*ind))
			}
			Ok(cap)
		}
	}
}
