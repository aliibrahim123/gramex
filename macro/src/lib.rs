mod gen_matcher;
mod gen_types;
mod parse;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::{
	Ident,
	parse::{ParseBuffer, Parser},
	parse_macro_input,
};

use crate::{
	gen_matcher::{Ctx, gen_expr, gen_matcher_expr, gen_term},
	gen_types::{find_unallowed_capture, resolve_types_expr, resolve_types_macro},
	parse::{
		GramexMacro, Matcher, MatcherExpr, parse_gramex_macro, parse_matcher, parse_matcher_expr,
	},
};
#[proc_macro]
pub fn gramex(input: TokenStream) -> TokenStream {
	let mut module = parse_macro_input!(input with parse_gramex_macro);
	let mut mod_def = resolve_types_macro(&mut module).unwrap_or_else(|e| e.to_compile_error());

	let GramexMacro { matched_type, mod_name, use_decls, mod_vis, terms } = module;
	for term in &terms {
		let captures_mod = &format_ident!("{}_captures", term.name);
		let mut ctx = Ctx { captures_mod, match_target: &matched_type, mat_label_id: 0 };
		mod_def.append_all(gen_term(term, &mut ctx));
	}

	let mut uses = quote! {};
	for use_decl in &use_decls {
		ToTokens::to_tokens(use_decl, &mut uses);
	}

	match mod_name {
		Some(name) => quote! { #[allow(unused, nonstandard_style)]
			#mod_vis mod #name { use super::*; #uses #mod_def }
		},
		None => mod_def,
	}
	.into()
}

macro_rules! parse_input {
	($input:ident with |$buf:ident| $parser:expr) => {
		match Parser::parse(|$buf: &ParseBuffer| $parser, $input) {
			Ok(matcher) => matcher,
			Err(err) => return err.to_compile_error().into(),
		}
	};
}

/// hallo
#[proc_macro]
pub fn try_match(input: TokenStream) -> TokenStream {
	let MatcherExpr { expr, matched_type, value } =
		parse_input!(input with |input| parse_matcher_expr(input, true));
	let cap_mod = Ident::new("captures", Span::call_site());
	let mod_def =
		resolve_types_expr(&expr, &matched_type, &cap_mod).unwrap_or_else(|e| e.to_compile_error());

	let mut ctx = Ctx { captures_mod: &cap_mod, match_target: &matched_type, mat_label_id: 0 };
	let matcher = gen_matcher_expr(&expr, &mut ctx);

	quote! { {
		let _value = #value;
		#[allow(unused, nonstandard_style)] let res = 'mat_0: { #mod_def #matcher };
		res
	} }
	.into()
}
#[proc_macro]
pub fn matches(input: TokenStream) -> TokenStream {
	let MatcherExpr { expr, matched_type, value } =
		parse_input!(input with |input| parse_matcher_expr(input, false));

	if let Err(err) = find_unallowed_capture(&expr) {
		return err.to_compile_error().into();
	}

	let ident = Ident::new("how_did_i_get_here", Span::call_site());
	let mut ctx = Ctx { captures_mod: &ident, match_target: &matched_type, mat_label_id: 0 };
	let matcher = gen_expr(&expr, &mut ctx);

	quote! { {
		let _value = #value;
		let ind = &mut 0;
		let status = &mut ::gramex::MatchStatus::default();
		#[allow(unused, nonstandard_style)] let res = #matcher;
		if *ind != <_ as ::gramex::MatchAble>::len(_value) { false }
		else { res == ::gramex::MatchSignal::Matched }
	} }
	.into()
}
#[proc_macro]
pub fn matcher(input: TokenStream) -> TokenStream {
	let Matcher { expr, matched_type } = parse_macro_input!(input with parse_matcher);

	if let Err(err) = find_unallowed_capture(&expr) {
		return err.to_compile_error().into();
	}

	let ident = Ident::new("how_did_i_get_here", Span::call_site());
	let mut ctx = Ctx { captures_mod: &ident, match_target: &matched_type, mat_label_id: 0 };
	let matcher = gen_expr(&expr, &mut ctx);

	quote! { {
		#[allow(unused, nonstandard_style)]
		|_value: &#matched_type, ind: &mut usize, status: &::gramex::MatchStatus| {
			let status = &mut status.clone(); #matcher
		}
	} }
	.into()
}
