use chunked_quote::quote;
use proc_macro::TokenStream;
use proc_macro2::Span;

use crate::{
	analyze::{CapMod, analyze_term},
	cursor::Cursor,
	parse::{parse_expr, parse_grammer_decl},
};

mod analyze;
mod cursor;
mod parse;

#[proc_macro]
pub fn example(tokens: TokenStream) -> TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(tokens.into(), Span::call_site(), &mut errors);
	let res = format!("{:#?}", parse_expr(&mut cur));
	quote! {{ #for e in errors #{#e} #res }}.into()
}

#[proc_macro]
pub fn gramex(input: TokenStream) -> TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let mut decl = parse_grammer_decl(&mut cur);

	let mut cap_mod = CapMod::default();
	let mut ctx = analyze::Context {
		capture_mod: Some(&mut cap_mod),
		errors: &mut errors,
		matched_type: Some(&decl.matched_type),
	};
	for term in &mut decl.terms {
		analyze_term(term, &mut ctx);
	}

	let res = format!("{decl:#?}");
	quote! {
		#for e in errors #{#e}
		mod captures {use super::*; #{cap_mod.stream}}
		const res: &'static str = #res;
	}
	.into()
}
