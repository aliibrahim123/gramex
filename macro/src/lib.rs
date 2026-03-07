mod gen_matcher;
mod gen_types;
mod parse;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::{Ident, parse_macro_input};

use crate::{
	gen_matcher::{Ctx, gen_matcher, gen_matcher_expr, gen_term},
	gen_types::{resolve_types_expr, resolve_types_macro},
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
fn match_expr(input: TokenStream, suffix: proc_macro2::TokenStream) -> TokenStream {
	let MatcherExpr { expr, matched_type, value } =
		parse_macro_input!(input with parse_matcher_expr);
	let cap_mod = Ident::new("captures", Span::call_site());
	let mod_def =
		resolve_types_expr(&expr, &matched_type, &cap_mod).unwrap_or_else(|e| e.to_compile_error());

	let mut ctx = Ctx { captures_mod: &cap_mod, match_target: &matched_type, mat_label_id: 0 };
	let matcher = gen_matcher_expr(&expr, &mut ctx);

	quote! { match AsRef::<#matched_type>::as_ref(&#value) {
		value => {
			#[allow(unused, nonstandard_style)] let res = 'mat_0: { #mod_def #matcher };
			res
		}
	} #suffix }
	.into()
}
/// hallo
#[proc_macro]
pub fn try_match(input: TokenStream) -> TokenStream {
	match_expr(input, quote! {})
}
#[proc_macro]
pub fn matches(input: TokenStream) -> TokenStream {
	match_expr(input, quote! { .is_ok() })
}
#[proc_macro]
pub fn matcher(input: TokenStream) -> TokenStream {
	let Matcher { expr, matched_type } = parse_macro_input!(input with parse_matcher);
	let cap_mod = Ident::new("captures", Span::call_site());
	let mod_def =
		resolve_types_expr(&expr, &matched_type, &cap_mod).unwrap_or_else(|e| e.to_compile_error());

	let mut ctx = Ctx { captures_mod: &cap_mod, match_target: &matched_type, mat_label_id: 0 };
	let matcher_body = gen_matcher(&expr, &mut ctx);

	quote! { {
		#mod_def
		fn _as <F: for<'a> Fn(&'a #matched_type, &mut usize, &::gramex::MatchStatus)
			-> ::gramex::MatchResult<captures::Root<'a>>> (f: F) -> F { f }
		_as (|value, ind, status| { #matcher_body Ok(unsafe {  cap_root.unwrap_unchecked() }) })
	} }
	.into()
}
