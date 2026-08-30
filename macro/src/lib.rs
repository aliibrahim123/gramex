use chunked_quote::quote;
use proc_macro::TokenStream;
use proc_macro2::Span;

use crate::{
	capture::{CapMod, analyze_term},
	cursor::Cursor,
	generate::gen_term,
	parse::{parse_expr, parse_grammer_decl},
};

mod capture;
mod cursor;
mod generate;
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
	let mut ctx = capture::Context {
		capture_mod: Some(&mut cap_mod),
		errors: &mut errors,
		matched_type: Some(&decl.matched_type),
	};
	let mut stream = TokenStream::new().into();
	for term in &mut decl.terms {
		analyze_term(term, &mut ctx);
		gen_term(&mut stream, term, &decl.matched_type);
	}

	quote! {
		#for e in errors #{#e}
		# #[allow(nonstandard_style, unused_imports)]
		pub mod captures {
			use super::*; use ::gramex::__private as __; #{cap_mod.stream}
		}
		#stream
	}
	.into()
}
