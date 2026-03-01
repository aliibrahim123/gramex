mod gen_matcher;
mod gen_types;
mod parse;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Pat, parse_macro_input};

use crate::{gen_types::resolve_types_macro, parse::parse_gramex_macro};

#[proc_macro]
pub fn make(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input with Pat::parse_multi);
	let res = quote! {
		pub fn example (inp: i64) -> bool {
			matches!(inp, #input)
		}
	};
	res.into()
}

#[proc_macro]
pub fn test_a(input: TokenStream) -> TokenStream {
	let mut input = parse_macro_input!(input with parse_gramex_macro);
	resolve_types_macro(&mut input).unwrap_or_else(|e| e.to_compile_error()).into()
}
