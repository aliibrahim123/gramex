use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, quote};

use crate::{
	gen_matcher,
	parse::{Atom, Expr, Repetition},
};

fn gen_atom(atom: &Atom) -> TokenStream {
	fn match_by(t: impl ToTokens) -> TokenStream {
		quote! { gramex::MatchAble::match_by(value, #t, ind, status) }
	}
	match atom {
		Atom::Literal(lit) => match_by(lit),
		Atom::Term(path) => match_by(path),
		Atom::Block(block) => match_by(block),
		Atom::Any => quote! {{ *ind += 1; gramex::MatchSignal::Matched }},
		Atom::Group(expr) => gen_expr(expr),
		Atom::Call { path, args } => {
			let mut args_res = quote! {};
			for arg in args {
				let mat = gen_matcher(arg);
				args_res.append_all(quote! { |value, ind, status| #mat, });
			}
			quote! { #path(value, #args_res, ind, status) }
		}
	}
}

fn gen_rep(repetition: &Repetition, matcher: TokenStream) -> TokenStream {
	if *repetition == Repetition::Once {
		return matcher;
	} else if *repetition == Repetition::Optional {
		return quote! { 'mat: {
			let start_ind = *ind;
			let status = gramex::MatchStatus { in_main_path: false, ..status };
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { *ind = start_ind };
			gramex
		}};
	};

	let Repetition(start, end) = *repetition;
	let mut match_block = quote! { start_ind = *ind; };
	if start == 0 {
		match_block.append_all(quote! {
			let state = &gramex::MatchStatus { in_main_path: false, ..status };
		});
	} else {
		match_block.append_all(quote! {
			let state = if iter < #start { state } else {
				&gramex::MatchStatus { in_main_path: false, ..status };
			}
		});
	}

	match_block.append_all(quote! {
		let sig = #matcher;
		let matched = sig == gramex::MatchSignal::Matched;
	});

	if end != u32::MAX {
		match_block.append_all(quote! {
			if iter >= #end {
				*ind = start_ind;
				if matched { break 'mat gramex::MatchSignal::MisMatched }
				else { break 'mat gramex::MatchSignal::Matched }
			};
		});
	}

	let mut mismatch_block = quote! {};
	if start != 0 {
		mismatch_block.append_all(quote! {
			if iter < #start { break 'mat gramex::MatchSignal::MisMatched };
		});
	}
	mismatch_block.append_all(quote! {
		*ind = start_ind;
		break 'mat gramex::MatchSignal::Matched
	});
	match_block.append_all(quote! { if !matched { #mismatch_block } });

	quote! { 'mat: for iter in .. { #match_block } }
}

fn gen_unit(unit: &Expr) -> TokenStream {
	let Expr::Unit { not, near, repetition, atom } = unit else { unreachable!() };
	fn gen_forked_match(matcher: TokenStream, mapper: TokenStream) -> TokenStream {
		quote! { 'mat: {
			let mut ind = &mut *ind;
			let status = &gramex::MatchStatus { in_main_path: false, ..status };
			let sig = #matcher,
			break 'mat #mapper
		} }
	}
	let matcher = gen_atom(atom);
	if *near {
		let mapper = if *not {
			quote! { match sig != gramex::MatchSignal::Matched {
				true => gramex::MatchSignal::Matched,
				false => gramex::MatchSignal::MisMatched
			} }
		} else {
			quote! { match sig {
				gramex::MatchSignal::Matched => gramex::MatchSignal::Matched,
				err => err
			} }
		};
		gen_forked_match(gen_rep(repetition, matcher), mapper)
	} else if *not {
		let mapper = quote! { match sig != gramex::MatchSignal::Matched {
			true => { *ind += 1; gramex::MatchSignal::Matched },
			false => gramex::MatchSignal::MisMatched
		} };
		gen_rep(repetition, gen_forked_match(matcher, mapper))
	} else {
		gen_rep(repetition, matcher)
	}
}

fn gen_range(range: &Expr) -> TokenStream {
	let Expr::Range(lh, rh) = range else { unreachable!() };
	fn gen_atom(atom: &Atom) -> TokenStream {
		match atom {
			Atom::Term(term) => quote! { #term },
			Atom::Literal(lit) => quote! { #lit },
			Atom::Block(block) => quote! { #block },
			Atom::Group(expr) => match &**expr {
				Expr::Unit { atom, .. } => gen_atom(atom),
				_ => unreachable!(),
			},
			_ => unreachable!(),
		}
	}
	let (lh, rh) = (gen_atom(lh), gen_atom(rh));
	quote! { gramex::MatchAble::match_by(value, #lh..#rh, ind, status) }
}

fn gen_seq(seq: &Expr) -> TokenStream {
	let Expr::Seq(exprs) = seq else { unreachable!() };
	let mut match_block = quote! {};
	for expr in exprs {
		let matcher = gen_expr(expr);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	quote! { 'mat: { #match_block; gramex::MatchSignal::Matched } }
}

fn gen_or(expr: &Expr) -> TokenStream {
	let Expr::Or(exprs) = expr else { unreachable!() };
	let mut match_block = quote! {
		let start_ind = *ind;
		let start_status = status;
		let status = &gramex::MatchStatus { in_main_path: false, ..start_status };
	};
	for expr in &exprs[0..exprs.len() - 1] {
		let matcher = gen_expr(expr);
		match_block.append_all(quote! {
			let sig = #matcher;
			if sig == gramex::MatchSignal::Matched { break 'mat sig }
			*ind = start_ind;
		});
	}
	let last_matcher = gen_expr(exprs.last().unwrap());
	match_block.append_all(quote! { status = start_status; #last_matcher });
	quote! { 'mat: { #match_block } }
}

fn gen_and(expr: &Expr) -> TokenStream {
	let Expr::And(exprs) = expr else { unreachable!() };
	let primary_matcher = gen_expr(&exprs[0]);
	let mut match_block = quote! {
		let start_ind = *ind;
		let sig = #primary_matcher;
		if sig != gramex::MatchSignal::Matched { break 'mat sig }
		let value = gramex::MatchAble::slice(value, 0..*ind);
	};
	for expr in &exprs[1..] {
		let matcher = gen_expr(expr);
		match_block.append_all(quote! {
			let ind = &mut start_ind.clone();
			let sig = #matcher;
			if sig != gramex::MatchSignal::Matched { break 'mat sig }
		});
	}
	quote! { 'mat: { #match_block; gramex::MatchSignal::Matched } }
}

fn gen_capture(expr: &Expr) -> TokenStream {
	let Expr::Capture { ident, rep, ty, conv, typeid, expr } = expr else { unreachable!() };
	todo!()
}

pub fn gen_expr(expr: &Expr) -> TokenStream {
	match expr {
		Expr::Unit { .. } => gen_unit(expr),
		Expr::Range(_, _) => gen_range(expr),
		Expr::Seq(_) => gen_seq(expr),
		Expr::Or(_) => gen_or(expr),
		Expr::And(_) => gen_and(expr),
		Expr::Capture { .. } => gen_capture(expr),
	}
}
pub fn gen_matcher(expr: &Expr) -> TokenStream {
	todo!()
}
