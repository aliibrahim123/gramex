mod parse;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Pat, parse_macro_input};

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
