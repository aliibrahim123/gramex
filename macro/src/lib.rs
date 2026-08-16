use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;

use crate::{cursor::Cursor, parse::parse_expr};

mod cursor;
mod parse;

#[proc_macro]
pub fn example(tokens: TokenStream) -> TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(tokens.into(), Span::call_site(), &mut errors);
	let res = format!("{:#?}", parse_expr(&mut cur));
	quote! {{#(#errors)* #res }}.into()
}
