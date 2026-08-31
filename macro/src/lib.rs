use std::sync::atomic::{AtomicI32, Ordering};

use chunked_quote::quote;
use proc_macro2::{Span, TokenStream};

use crate::{
	capture::{CapMod, analyze_expr, analyze_matcher, analyze_term, forbid_captures},
	cursor::{Cursor, ident},
	generate::{gen_imports, gen_match_expr, gen_matcher, gen_term},
	parse::{
		Capture, Expr, MatchExpr, parse_grammer_decl, parse_match_expr, parse_matcher,
	},
};

mod capture;
mod cursor;
mod generate;
mod parse;

#[proc_macro]
pub fn gramex(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let mut decl = parse_grammer_decl(&mut cur);

	let mut cap_mod = CapMod::default();
	let mut ctx = capture::Context {
		capture_mod: Some(&mut cap_mod),
		errors: &mut errors,
		matched_type: Some(&decl.matched_type),
	};
	let mut stream = TokenStream::new();
	for term in &mut decl.terms {
		analyze_term(term, &mut ctx);
		gen_term(&mut stream, term, &decl.matched_type);
	}

	static CUR_ID: AtomicI32 = AtomicI32::new(0);
	let id = CUR_ID.fetch_add(1, Ordering::Relaxed);
	quote! {
		#for e in errors #{#e}
		# #[doc(hidden)]
		mod #{ident!("gram_def_{id}")} {
			use super::*;
			#do { gen_imports(__stream) }
			#{cap_mod.stream}
			#stream
		}
		pub use #{ident!("gram_def_{id}")}::*;
	}
	.into()
}

#[proc_macro]
pub fn matcher(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let mut matcher = parse_matcher(&mut cur, false);
	let mut ctx =
		capture::Context { capture_mod: None, errors: &mut errors, matched_type: None };
	analyze_matcher(&mut matcher, &mut ctx);

	quote! { {
		#for e in errors #{#e}
		#do { gen_imports(__stream) }
		#do { gen_matcher(__stream, &matcher) }
	} }
	.into()
}

fn match_expr(input: TokenStream, capture: bool, error: bool) -> TokenStream {
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let MatchExpr { matched_type, value, mut expr } = parse_match_expr(&mut cur);

	let mut ctx = capture::Context {
		capture_mod: None,
		errors: &mut errors,
		matched_type: matched_type.as_ref(),
	};
	if capture {
		let cap = Capture { ident: ident!("root"), expr, ..Default::default() };
		expr = Expr::Capture(Box::new(cap));
		analyze_expr(&mut expr, false, &mut ctx);
	} else {
		forbid_captures(&expr, &mut errors);
	}
	quote! { {
		#for e in errors #{#e}
		#do { gen_imports(__stream) }
		#do { gen_match_expr(__stream, capture, error, value, &expr) }
	} }
}

#[proc_macro]
pub fn matches(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), false, false).into()
}
#[proc_macro]
pub fn check(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), false, true).into()
}
#[proc_macro]
pub fn try_match(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, false).into()
}
#[proc_macro]
pub fn parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, true).into()
}
