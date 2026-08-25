use chunked_quote::chunk;
use proc_macro2::{Ident, Punct, Spacing::Joint, Span, TokenStream};
use quote::ToTokens;

use crate::{
	cursor::Error,
	parse::{Atom, Expr},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
	Test,
	Check,
	Capture,
}

#[derive(Debug)]
pub struct Ctx {
	mode: Mode,
	cur_match_block: u64,
}

fn is_ok(mut stream: &mut TokenStream, ctx: &Ctx) {
	chunk!(stream, #match ctx.mode {
		Mode::Test => #{ res },
		_ => #{ res.is_ok() },
	});
}
fn is_err(mut stream: &mut TokenStream, ctx: &Ctx) {
	chunk!(stream, #match ctx.mode {
		Mode::Test => #{ !res },
		_ => #{ res.is_err() },
	});
}
fn match_block(mut stream: &mut TokenStream, ctx: &Ctx) {
	chunk!(stream,
		#{Punct::new('\'', Joint)}
		#{Ident::new(&format!("mat_{}", ctx.cur_match_block), Span::call_site())}
	);
}
fn _break(mut stream: &mut TokenStream, what: impl Fn(&mut TokenStream), ctx: &Ctx) {
	chunk!(stream, break #do {match_block(stream, ctx)} )
}

fn gen_atom(stream: &mut TokenStream, atom: &Atom, ctx: &mut Ctx) {
	/*match atom {
		Atom::Any => chunk!(stream, {
			let res = <_ as ::gramex::MatchAble>::get_n(_value, off, 1);
			if #do {is_err(stream, ctx)} {

			}
		}),
	}*/
}

fn gen_unit(stream: &mut TokenStream, expr: &Expr, ctx: &mut Ctx) {
	let Expr::Unit { not, near, rep, atom } = expr else { unreachable!() };
}

fn gen_expr(stream: &mut TokenStream, expr: &Expr, ctx: &mut Ctx) {
	match expr {
		Expr::Unit { .. } => gen_unit(stream, expr, ctx),
		_ => todo!(),
	}
}
