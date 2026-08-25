use chunked_quote::chunk;
use proc_macro2::{Ident, Punct, Spacing::Joint, Span, TokenStream};
use quote::ToTokens;

use crate::parse::{Atom, Expr, Matcher, Rep};

#[derive(Debug, Clone, Copy)]
struct BlockLable(u64);
impl ToTokens for BlockLable {
	fn to_tokens(&self, mut tokens: &mut TokenStream) {
		chunk!(tokens,
			#{Punct::new('\'', Joint)}
			#{Ident::new(&format!("mat_{}", self.0), Span::call_site())}
		);
	}
}
impl BlockLable {
	fn next(&self) -> BlockLable {
		BlockLable(self.0 + 1)
	}
}

#[derive(Debug, Clone, Copy)]
struct Mode {
	capture: bool,
	error: bool,
}
impl Mode {
	fn no_cap(&self) -> Mode {
		Mode { capture: false, ..*self }
	}
	fn no_err(&self) -> Mode {
		Mode { error: false, ..*self }
	}
}
impl ToTokens for Mode {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		chunk!(tokens, M
			#if !self.capture #{ ::WithoutCapture }
			#if !self.error #{ ::WithoutError }
		);
	}
}

#[derive(Debug, Copy, Clone)]
struct Context {
	label: BlockLable,
	mode: Mode,
}
impl Context {
	fn next_label(&self) -> Context {
		Context { label: self.label.next(), ..*self }
	}
	fn no_err(&self) -> Context {
		Context { mode: self.mode.no_err(), ..*self }
	}
}

fn fork(mut stream: &mut TokenStream, ctx: Context, item: impl Fn(&mut TokenStream, Context)) {
	let child_ctx = ctx.next_label();
	chunk!(stream, #{ctx.label} {
		#do { item(stream, child_ctx) }
		Ok(())
	})
}

fn gen_expected_atom(stream: &mut TokenStream, atom: &Atom) {
	match atom {
		Atom::Any => chunk!(stream, ::gramex::Expected::A("anything".into())),
		Atom::Matcher(matcher) => {
			chunk!(stream, <_ as ::gramex::Matcher<_>>::expected(&(#matcher)))
		}
		Atom::Group(expr) => gen_expected(stream, expr),
		Atom::Call { .. } => chunk!(stream, ::gramex::Expected::None),
	}
}

fn gen_expected_not(mut stream: &mut TokenStream, thing: impl Fn(&mut TokenStream)) {
	chunk!(stream, #do{}
		::gramex::Expected::A(::gramex::__private::format!(
			"not {}", #do { thing(stream) }.to_string()
		).into())
	);
}

fn gen_expected(mut stream: &mut TokenStream, expr: &Expr) {
	match expr {
		Expr::Unit { not: true, atom, .. } => {
			gen_expected_not(stream, |stream| gen_expected_atom(stream, atom))
		}
		Expr::Unit { atom, .. } => gen_expected_atom(stream, atom),
		Expr::Range(left, right) => {
			chunk!(stream, <_ as ::gramex::Matcher<_>>::expected(&(#left)..=(#right)))
		}
		Expr::And(exprs) | Expr::Seq(exprs) => gen_expected(stream, &exprs[0]),
		Expr::Imply { cond, .. } => gen_expected(stream, cond),
		Expr::Capture(cap) => gen_expected(stream, &cap.expr),
		Expr::Error => {}
		Expr::Or(exprs) => chunk!(stream, #do {}
			::gramex::Expected::OneOf(::gramex::__private::vec![
				#for expr in exprs #{
					#do { gen_expected(stream, expr) }.to_string().into(),
				}
			])
		),
	}
}

fn gen_atom(mut stream: &mut TokenStream, atom: &Atom, ctx: Context) {
	match atom {
		Atom::Any => chunk!(stream,
			if !<_ as ::gramex::MatchAble>::skip_n(&__value, __off, 1) {
				break #{ctx.label} M::err(||
					::gramex::MatchError::incomplete(
						::gramex::Expected::A("something".into()), *__off
					)
				);
			};
		),
		Atom::Matcher(matcher) => chunk!(stream,
			if let Err(err) = <_ as ::gramex::Matcher<_>>
				::do_match::<#{ctx.mode.no_cap()}>(&(#matcher), &__value, __off)
			{ break #{ctx.label} err }
		),
		Atom::Group(expr) => gen_expr(&mut stream, expr, ctx),
		Atom::Call { path, args } => chunk!(stream,
			if let Err(err) = <_ as ::gramex::Matcher<_>>::do_match::<#{ctx.mode.no_cap()}>(
				#for part in path #{ #part }(
					#for arg in args #{ #do { gen_matcher(stream, arg, ctx) }, }
				),
				&__value, __off
			) { break #{ctx.label} err }
		),
	}
}

fn gen_rep_complex(
	stream: &mut TokenStream, rep: Rep, ctx: Context, item: impl Fn(&mut TokenStream, Context),
) {
	let Rep(start, end) = rep;
	chunk!(stream, loop {
		let mut __iter = 0;
		let __start = *__off;
		let __res = #do { fork(stream, ctx, item) };
		if let Err(err) = __res {
			#if start != 0 #{
				if __iter < #start { break #{ctx.label} err }
			}
			*__off = __start;
			break
		}
		__iter += 1;
		#if end != u32::MAX #{
			if __iter == #end { break }
		}
	})
}

fn gen_rep(
	stream: &mut TokenStream, rep: Rep, ctx: Context, item: impl Fn(&mut TokenStream, Context),
) {
	if rep == Rep::ONCE {
		item(stream, ctx);
	} else if rep == Rep::OPTIONAL {
		chunk!(stream, {
			let __orig = __off;
			let __off = &mut *__off;
			let __res = #do { fork(stream, ctx.no_err(), item) };
			if __res.is_ok() { *__orig = *__off }
		})
	} else {
		gen_rep_complex(stream, rep, ctx, item)
	}
}

fn gen_unit_near(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, rep, atom, .. } = expr else { unreachable!() };
	let gen_logic =
		|stream: &mut _, ctx| gen_rep(stream, *rep, ctx, |stream, ctx| gen_atom(stream, atom, ctx));
	chunk!(stream, {
		let __orig = __off;
		let __off = &mut *__off;
		#if *not #{
			let __res = #do { fork(stream, ctx.no_err(), gen_logic) };
			if __res.is_ok() {
				break #{ctx.label} M::err(||
					::gramex::MatchError::mismatch(#do {
						gen_expected_not(stream, |stream| gen_expected_atom(stream, atom))
					}, *__off )
				)
			}
		}
		#else #{ #do { gen_logic(stream, ctx) } }
	})
}

fn gen_unit_not(stream: &mut TokenStream, atom: &Atom, rep: Rep, ctx: Context) {
	gen_rep(stream, rep, ctx, |stream, ctx| {
		chunk!(stream, {
			let __orig = __off;
			let __off = &mut *__off;
			let __res = #do { fork(stream, ctx.no_err(), |stream, ctx| gen_atom(stream, atom, ctx)) };
			match __res {
				Ok(()) => break #{ctx.label} M::err(||
					::gramex::MatchError::mismatch(#do {
						gen_expected_not(stream, |stream| gen_expected_atom(stream, atom))
					}, *__off )
				),
				Err(_) if *off == <_ as ::gramex::MatchAble>::len(&__value) =>
					break #{ctx.label} M::err(||
						::gramex::MatchError::incomplete(#do {
							gen_expected_not(stream, |stream| gen_expected_atom(stream, atom))
						}, *__off )
					),
				_ => *__orig += 1,
			}
		})
	});
}

fn gen_unit(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, near, rep, atom } = expr else { unreachable!() };
	if *not == false && *near == false && matches!(atom, Atom::Any) && rep.is_exact() {
		chunk!(stream,
			if !<_ as ::gramex::MatchAble>::skip_n(&__value, __off, #{rep.0}) {
				break #{ctx.label} M::err(||
					::gramex::MatchError::incomplete(::gramex::Expected::A("something"), *__off)
				);
			};
		);
	} else if *near {
		gen_unit_near(stream, expr, ctx);
	} else if *not {
		gen_unit_not(stream, atom, *rep, ctx);
	} else {
		gen_rep(stream, *rep, ctx, |stream, ctx| gen_atom(stream, atom, ctx));
	}
}

fn gen_expr(mut stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	match expr {
		Expr::Unit { .. } => gen_unit(stream, expr, ctx),
		Expr::Range(left, right) => chunk!(stream,
			if let Err(err) = <_ as ::gramex::Matcher<_>>
				::do_match::<#{ctx.mode.no_cap()}>(&(#left..=#right), &__value, __off)
			{ break #{ctx.label} err }
		),
		Expr::Seq(exprs) => {
			for expr in exprs {
				gen_expr(stream, expr, ctx);
			}
		}

		_ => todo!(),
	}
}

fn gen_matcher(stream: &mut TokenStream, matcher: &Matcher, ctx: Context) {
	todo!()
}
