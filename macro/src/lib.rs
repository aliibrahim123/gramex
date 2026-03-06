mod gen_matcher;
mod gen_types;
mod parse;

use proc_macro::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::parse_macro_input;

use crate::{
	gen_matcher::{Ctx, gen_term},
	gen_types::resolve_types_macro,
	parse::{GramexMacro, parse_gramex_macro},
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
	#[allow(nonstandard_style)]
	match mod_name {
		Some(name) => quote! { #[allow(unused, nonstandard_style)]
			#mod_vis mod #name { use super::*; #uses #mod_def }
		},
		None => mod_def,
	}
	.into()
}
