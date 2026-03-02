mod gen_matcher;
mod gen_types;
mod parse;

use proc_macro::TokenStream;
use quote::{TokenStreamExt, format_ident, quote};
use syn::{Pat, parse_macro_input};

use crate::{
	gen_matcher::{Ctx, gen_term},
	gen_types::resolve_types_macro,
	parse::parse_gramex_macro,
};

#[proc_macro]
pub fn gramex(input: TokenStream) -> TokenStream {
	let mut module = parse_macro_input!(input with parse_gramex_macro);
	let mut mod_def = resolve_types_macro(&mut module).unwrap_or_else(|e| e.to_compile_error());

	for term in &module.terms {
		let captures_mod = &format_ident!("{}_captures", term.name);
		mod_def.append_all(gen_term(term, &module.matched_type, &Ctx { captures_mod }));
	}

	match module.mod_name {
		Some(name) => quote! { mod #name { use super::*; #mod_def } },
		None => mod_def,
	}
	.into()
}
