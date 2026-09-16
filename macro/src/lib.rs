#![allow(clippy::match_bool)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::doc_markdown)]

use std::sync::atomic::{AtomicI32, Ordering};

use chunked_quote::quote;
use proc_macro2::{Span, TokenStream};

use crate::{
	capture::{CapMod, analyze_matcher, analyze_root_cap, analyze_term, forbid_captures},
	cursor::{Cursor, ident},
	generate::{
		CursorOp, gen_cursor_op, gen_enum_matcher, gen_imports, gen_match_expr,
		gen_match_map, gen_matcher, gen_term,
	},
	parse::{
		Capture, Expr, MatchExpr, MatchMap, parse_enum_matcher, parse_grammer_decl,
		parse_match_expr, parse_match_map, parse_matcher,
	},
};

mod capture;
mod cursor;
mod generate;
mod parse;

#[proc_macro]
pub fn gramex(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	// parse
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let mut decl = parse_grammer_decl(&mut cur);

	// analyze
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

	// gen
	static CUR_ID: AtomicI32 = AtomicI32::new(0);
	let id = CUR_ID.fetch_add(1, Ordering::Relaxed);
	quote! {
		#for err in errors #{ #err }
		// detecated module to have global imports
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
	// parse
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let mut matcher = parse_matcher(&mut cur, false);

	// analyze
	let mut ctx = capture::Context::new_expr(None, &mut errors);
	analyze_matcher(&mut matcher, &mut ctx);

	// gen
	quote! { {
		#for err in errors #{ #err }
		#do { gen_imports(__stream) }
		#do { gen_matcher(__stream, &matcher) }
	} }
	.into()
}

fn match_expr(
	input: TokenStream, capture: bool, is_cur_op: bool,
	gen_: impl Fn(&mut TokenStream, &TokenStream, &Expr),
) -> TokenStream {
	// parse
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input, Span::call_site(), &mut errors);
	let Some(MatchExpr { matched_type, value, mut expr }) =
		parse_match_expr(&mut cur, is_cur_op)
	else {
		return quote! {{
			#for err in errors #{ #err }
			unreachable!()
		}};
	};

	// analyze
	if capture {
		let mut ctx = capture::Context::new_expr(matched_type.as_ref(), &mut errors);
		let mut cap = Capture { ident: ident!("root"), expr, ..Default::default() };
		analyze_root_cap(&mut cap, false, &mut ctx);
		expr = Expr::Capture(Box::new(cap));
	} else {
		forbid_captures(&expr, &mut errors);
	}

	// gen
	quote! { {
		#for err in errors #{ #err }
		#do { gen_imports(__stream) }
		#do { gen_(__stream, &value, &expr) }
	} }
}

#[proc_macro]
pub fn matches(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), false, false, |stream, value, expr| {
		gen_match_expr(stream, false, false, value, expr);
	})
	.into()
}
#[proc_macro]
pub fn check(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), false, false, |stream, value, expr| {
		gen_match_expr(stream, false, true, value, expr);
	})
	.into()
}
#[proc_macro]
pub fn try_match(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, false, |stream, value, expr| {
		gen_match_expr(stream, true, false, value, expr);
	})
	.into()
}
#[proc_macro]
pub fn parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, false, |stream, value, expr| {
		gen_match_expr(stream, true, true, value, expr);
	})
	.into()
}

#[proc_macro]
pub fn eat(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, true, |stream, cur, expr| {
		gen_cursor_op(stream, CursorOp::Eat, cur, expr);
	})
	.into()
}
#[proc_macro]
pub fn try_eat(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), true, true, |stream, cur, expr| {
		gen_cursor_op(stream, CursorOp::TryEat, cur, expr);
	})
	.into()
}
#[proc_macro]
pub fn test(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match_expr(input.into(), false, true, |stream, cur, expr| {
		gen_cursor_op(stream, CursorOp::Test, cur, expr);
	})
	.into()
}

#[proc_macro]
pub fn match_map(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	// parse
	let mut errors = Vec::new();
	let mut cur = Cursor::new(input.into(), Span::call_site(), &mut errors);
	let Some(MatchMap { cursor, mut arms, else_ }) = parse_match_map(&mut cur) else {
		return quote! {{
			#for err in errors #{ #err }
			unreachable!()
		}}
		.into();
	};

	// analyze
	for arm in &mut arms {
		let mut ctx = capture::Context::new_expr(None, &mut errors);
		analyze_root_cap(arm, false, &mut ctx);
	}

	// gen
	quote! { 'mat_0: {
		#for err in errors #{ #err }
		#do { gen_imports(__stream) }
		#do { gen_match_map(__stream, &cursor, &arms, &else_) }
	} }
	.into()
}

#[proc_macro_attribute]
pub fn derive_enum_matcher(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	// parse
	let mut errors = Vec::new();
	let item = TokenStream::from(item);
	let Some(matcher) = parse_enum_matcher(attr.into(), item.clone(), &mut errors) else {
		return quote! { #for err in errors #{ #err } #item }.into();
	};

	// gen
	quote! {
		#for err in errors #{ #err }
		#do { gen_enum_matcher(__stream, &matcher) }
		#item
	}
	.into()
}
