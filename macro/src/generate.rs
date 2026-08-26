use chunked_quote::{chunk, quote};
use proc_macro2::{Ident, Punct, Spacing::Joint, Span, TokenStream};
use quote::ToTokens;

use crate::{
	capture::{CapChild, CapContainer, CapInfo, CapKind, pascal_case},
	cursor::ident,
	parse::{Atom, Capture, Expr, Matcher, Rep, Term},
};

#[derive(Debug, Clone, Copy)]
struct BlockLable(u64);
impl ToTokens for BlockLable {
	fn to_tokens(&self, mut tokens: &mut TokenStream) {
		chunk!(tokens,
			#{Punct::new('\'', Joint)}
			#{ident!("mat_{}", self.0)}
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

fn fork(
	mut stream: &mut TokenStream, ctx: Context, item: impl Fn(&mut TokenStream, Context),
) {
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

#[rustfmt::skip]
fn gen_expected_or(mut stream: &mut TokenStream, exprs: &[Expr]) {
	chunk!(stream, ::gramex::Expected::OneOf(
		::gramex::__private::vec![
			#for expr in exprs #{
				#do { gen_expected(stream, expr) }.to_string().into(),
			}
		]
	));
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
		Expr::Or(exprs) => gen_expected_or(stream, exprs),
	}
}

fn error_not_atom(mut stream: &mut TokenStream, kind: &str, atom: &Atom) {
	chunk!(stream, ::gramex::MatchError::#{ident!("{kind}")}(#do {
		gen_expected_not(stream, |stream| gen_expected_atom(stream, atom))
	}, *__off ))
}

#[rustfmt::skip]
fn error_any(stream: &mut TokenStream) {
	chunk!(stream, ::gramex::MatchError::incomplete(
		::gramex::Expected::A("something".into()), *__off
	))
}

fn gen_atom(mut stream: &mut TokenStream, atom: &Atom, ctx: Context) {
	match atom {
		Atom::Any => chunk!(stream,
			if !<_ as ::gramex::MatchAble>::skip_n(&__value, __off, 1) {
				break #{ctx.label} #{ctx.mode}::err(|| #do { error_any(stream) });
			};
		),
		Atom::Matcher(matcher) => chunk!(stream,
			if let Err(err) = <_ as ::gramex::Matcher<_>>
				::do_match::<#{ctx.mode.no_cap()}>(&(#matcher), &__value, __off)
			{ break #{ctx.label} err }
		),
		Atom::Group(expr) => gen_expr(&mut stream, expr, ctx),
		Atom::Call { path, args } => chunk!(stream,
			if let Err(err) = <_ as ::gramex::Matcher<_>>
				::do_match::<#{ctx.mode.no_cap()}>
			(
				#for part in path #{ #part }(
					#for arg in args #{ #do { gen_matcher(stream, arg, ctx) }, }
				),
				&__value, __off
			) { break #{ctx.label} err }
		),
	}
}

fn gen_rep_complex(
	stream: &mut TokenStream, rep: Rep, ctx: Context,
	item: impl Fn(&mut TokenStream, Context),
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
	stream: &mut TokenStream, rep: Rep, ctx: Context,
	item: impl Fn(&mut TokenStream, Context),
) {
	if rep == Rep::ONCE {
		item(stream, ctx);
	} else if rep == Rep::OPTIONAL {
		chunk!(stream, {
			let __start = *__off;
			let __res = #do { fork(stream, ctx.no_err(), item) };
			if __res.is_err() { *__off = start }
		})
	} else {
		gen_rep_complex(stream, rep, ctx, item)
	}
}

fn gen_unit_near(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, rep, atom, .. } = expr else { unreachable!() };
	let gen_logic = |stream: &mut _, ctx| {
		gen_rep(stream, *rep, ctx, |stream, ctx| gen_atom(stream, atom, ctx))
	};
	chunk!(stream, {
		let __orig = __off;
		let __off = &mut *__off;
		#if *not #{
			let __res = #do { fork(stream, ctx.no_err(), gen_logic) };
			if __res.is_ok() {
				break #{ctx.label} #{ctx.mode}::err(||
					#do { error_not_atom(stream, "mismatch", atom) }
				)
			}
		}
		#else #{ #do { gen_logic(stream, ctx) } }
	})
}

fn gen_unit_not(stream: &mut TokenStream, atom: &Atom, rep: Rep, ctx: Context) {
	gen_rep(stream, rep, ctx, |stream, ctx| {
		chunk!(stream, {
			let __start = *__off;
			let __res = #do { fork(stream, ctx.no_err(),
				|stream, ctx| gen_atom(stream, atom, ctx))
			};
			match __res {
				Ok(()) => break #{ctx.label} #{ctx.mode}::err(||
					#do { error_not_atom(stream, "mismatch", atom)
				}),
				Err(_) if *off == <_ as ::gramex::MatchAble>::len(&__value) =>
					break #{ctx.label} #{ctx.mode}::err(||
						#do { error_not_atom(stream, "incomplete", atom) }
					),
				_ => *off = __start + 1,
			}
		})
	});
}

fn gen_unit(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, near, rep, atom } = expr else { unreachable!() };
	if *not == false && *near == false && matches!(atom, Atom::Any) && rep.is_exact() {
		chunk!(stream,
			if !<_ as ::gramex::MatchAble>::skip_n(&__value, __off, #{rep.0}) {
				break #{ctx.label} #{ctx.mode}::err(|| #do { error_any(stream) });
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

fn gen_or_branch(
	mut stream: &mut TokenStream, expr: &Expr, ind: usize,
	after: &impl Fn(&mut TokenStream, usize), new_ctx: Context,
) {
	chunk!(stream,
		let __res = #do { fork(stream, new_ctx.no_err(),
			|stream, ctx| gen_expr(stream, expr, ctx)
		) };
		if __res.is_ok() {
			#do { after(stream, ind) };
			break #{new_ctx.label} Ok(());
		}
	)
}

fn gen_or(
	mut stream: &mut TokenStream, exprs: &[Expr], ctx: Context,
	before: impl Fn(&mut TokenStream, usize), after: impl Fn(&mut TokenStream, usize),
) {
	let new_ctx = ctx.next_label();
	chunk!(stream, #{new_ctx.label}: {
		let __start = *__off;
		#for (ind, expr) in exprs.iter().enumerate() #{
			#do { before(stream, ind) };
			*off = __start;
			#if let Expr::Imply { cond, expr } = expr #{
				#do { gen_or_branch(stream, cond, ind, &|stream, ind| {
					gen_expr(stream, expr, ctx);
					after(stream, ind)
				}, new_ctx) }
			}
			#else #{
				#do { gen_or_branch(stream, expr, ind, &after, new_ctx) }
			}
		}
		break #{ctx.label} #{ctx.mode}::err(||
			let __expected = #do { gen_expected_or(stream, exprs) };
			if *off == <_ as ::gramex::MatchAble>::len(&__value) {
				::gramex::MatchError::incomplete(__expected)
			} else {
				::gramex::MatchError::mismatch(__expected)
			};
		);
	})
}

fn gen_and(stream: &mut TokenStream, exprs: &[Expr], ctx: Context) {
	chunk!(stream, {
		let __start = *__off;
		#do { gen_expr(stream, &exprs[0], ctx) }
		let __value = <_ as ::gramex::MatchAble>::slice(&__value, 0..*__off);
		let __off = &mut 0;
		#for expr in &exprs[1..] #{
			*__off = __start;
			#do { gen_expr(stream, expr, ctx) }
		}
	});
}

fn gen_imply(stream: &mut TokenStream, cond: &Expr, expr: &Expr, ctx: Context) {
	chunk!(stream, {
		let __start = *__off;
		let __res = #do { fork(stream, ctx,
			|stream, ctx| gen_expr(stream, cond, ctx.no_err()
		)) };
		if __res.is_ok() { #do { gen_expr(stream, expr, ctx) } }
		else { *__off = __start }
	})
}

fn gen_atomic_capture(mut stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	match expr {
		Expr::Unit { atom: Atom::Matcher(matcher), .. } => chunk!(stream,
			<_ as ::gramex::Matcher<_>>::do_match
			::<#{ctx.mode}>(&(#matcher), &__value, __off)
		),
		Expr::Unit { atom: Atom::Call { path, args }, .. } => chunk!(stream,
			<_ as ::gramex::Matcher<_>>::do_match::<#{ctx.mode}>(
				#for part in path #{ #part }(
					#for arg in args #{ #do { gen_matcher(stream, arg, ctx) }, }
				),
				&__value, __off
			)
		),
		Expr::Range(left, right) => chunk!(stream,
			<_ as ::gramex::Matcher<_>>::do_match
			::<#{ctx.mode}>(&(#left..=#right), &__value, __off)
		),
		_ => unreachable!(),
	}
}

fn gen_capture_unwrwap(
	mut stream: &mut TokenStream, ident: &Ident, container: CapContainer,
) {
	chunk!(stream,
		#{ident!("__cap__{ident}")}
		#if matches!(container, CapContainer::None | CapContainer::Vec) #{
			.unwrap()
		}
	)
}

fn gen_capture_set(mut stream: &mut TokenStream, ident: &Ident, container: CapContainer) {
	chunk!(stream,
		#{ident!("__cap__{ident}")} #match container {
			CapContainer::None | CapContainer::Option => #{ = Some(__cap) },
			CapContainer::Vec | CapContainer::OptionVec => #{ .get_or_insert_default().push(__cap) },
		};
	)
}

fn gen_capture_normal(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: Context,
) {
	let (is_atomic, need_from) = match &info.kind {
		CapKind::Atomic { need_from } => (true, need_from),
		CapKind::Normal { need_from } => (false, need_from),
		_ => unreachable!(),
	};

	chunk!(stream, {
		#if is_atomic #{
			let __cap = #do { gen_atomic_capture(stream, &cap.expr, ctx) }
		} #else #{
			#do { gen_expr(stream, &cap.expr, ctx) }
		}
		if #{ctx.mode}::DO_CAPTURE {
			#if !is_atomic #{
				let __cap =  <_ as ::gramex::MatchAble>::slice(&__value, __start..*__off);
			}
			#if *need_from #{
				let __cap = ::core::convert::Into<#{&info.resolved_type}>::into(__cap);
			}
			#if let Some(map) = &cap.map #{
				let #{&cap.ident} = __cap;
				let __cap = #map;
			}
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		}
	});
}

fn gen_capture_struct(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: Context,
) {
	let (fields, is_tuple) = match &info.kind {
		CapKind::Struct(fields) => (fields, false),
		CapKind::Tuple(fields) => (fields, true),
		_ => unreachable!(),
	};

	chunk!(stream, {
		let __start = *__off;
		#for CapChild { name, ..} in fields #{
			let #{ident!("__cap__{name}")} = None;
		}
		#do { gen_expr(stream, &cap.expr, ctx) }
		if #{ctx.mode}::DO_CAPTURE {
			#if let Some(map) = &cap.map #{
				#for field in fields #{
					let #{&field.name} = #do {
						gen_capture_unwrwap(stream, &field.name, field.container)
					};
				}
				let __cap = #map;
			} #else if is_tuple #{
				let __cap = (#for field in fields #{
					#do { gen_capture_unwrwap(stream, &field.name, field.container) },
				});
			} #else if fields.is_empty() #{
				let __cap = <_ as ::gramex::MatchAble>::slice(&__value, __start..*__off);
				let __cap = #{&info.resolved_type}(__cap);
			} #else #{
				let __cap = #{&info.resolved_type} {
					#for field in fields #{
						#{&field.name} = #do {
							gen_capture_unwrwap(stream, &field.name, field.container)
						},
					}
				};
			}
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		}
	});
}

fn gen_capture_enum(
	stream: &mut TokenStream, cap: &Capture, vars: &[Option<Ident>], info: &CapInfo,
	ctx: Context,
) {
	let Expr::Or(exprs) = &cap.expr else { unreachable!() };
	let before = |mut stream: &mut TokenStream, ind| {
		chunk!(stream,
			#if let Some(var) = &vars[ind] #{
				let #{ident!("__cap__{var}")} = None;
			}
		)
	};
	let after = |mut stream: &mut TokenStream, ind| {
		chunk!(stream, if #{ctx.mode}::DO_CAPTURE {
			let __cap = #{&info.resolved_type}::
			#match &vars[ind] {
				Some(var) => #{ #{pascal_case(var)} (#{ident!("__cap__{var}")}) },
				_ => #{None},
			}
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		})
	};
	gen_or(stream, exprs, ctx, before, after);
}

fn gen_capture(mut stream: &mut TokenStream, cap: &Capture, ctx: Context) {
	let Some(info) = &cap.info else { return gen_expr(stream, &cap.expr, ctx) };

	match &info.kind {
		CapKind::Atomic { .. } | CapKind::Normal { .. } => {
			gen_capture_normal(&mut stream, cap, info, ctx)
		}
		CapKind::Struct(_) | CapKind::Tuple(_) => {
			gen_capture_struct(&mut stream, cap, info, ctx)
		}
		CapKind::Enum(vars) => gen_capture_enum(&mut stream, cap, vars, info, ctx),
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
		Expr::Imply { cond, expr } => gen_imply(stream, cond, expr, ctx),
		Expr::Seq(exprs) => {
			for expr in exprs {
				gen_expr(stream, expr, ctx);
			}
		}
		Expr::And(exprs) => gen_and(stream, exprs, ctx),
		Expr::Or(exprs) => gen_or(stream, exprs, ctx, |_, _| (), |_, _| ()),
		Expr::Capture(cap) => gen_capture(stream, cap, ctx),
		Expr::Error => {}
	}
}

fn gen_term(mut stream: &mut TokenStream, term: &Term, matched_type: &TokenStream) {
	let actual_args = term.args.iter().filter(|arg| *arg != "_").collect::<Vec<_>>();

	let Expr::Capture(cap) = &term.expr else { unreachable!() };
	let (capture, container) = match &cap.info {
		Some(info) => (info.resolved_type.clone(), info.container),
		_ => (quote! { () }, CapContainer::None),
	};

	let matcher_ident = ident!("{}__Matcher", term.name);
	let ctx = Context { label: BlockLable(0), mode: Mode { capture: true, error: true } };
	chunk!(stream,
		#if term.args.is_empty() #{
			pub use #matcher_ident as #{&term.name};
		} #else #{
			pub fn #{&term.name}(
				#for arg in &term.args #{
					#arg: impl ::gramex::Matcher<#matched_type>
				}
			) -> impl ::gramex::Matcher<#matched_type> {
				#matcher_ident(
					#for arg in &actual_args #{ #arg, }
				)
			}
		}

		# #[doc(hidden)]
		pub struct #matcher_ident
		#if actual_args.len() > 0 #{
			(#for arg in &actual_args #{ #arg })
		};

		impl ::gramex::Matcher<#matched_type> for #matcher_ident {
			type Capture<'a> = #capture;
			fn do_match<'a, M: ::gramex::Mode>(
				&self, matched: &'a #matched_type, off: &mut usize,
			) -> MatchResult<Self::Capture<'a>, M> {
				#if actual_args.len() > 0 #{
					let Self(
						#for arg in actual_args #{ #arg, }
					) = self;
				}
				let mut __cap__root = None;
				#{ctx.label}: {
					#do { gen_expr(stream, &cap.expr, ctx) }
				}?;
				M::ok(|| #do { gen_capture_unwrwap(stream, &cap.ident, container) })
			}
		}
	);
}

fn gen_matcher(stream: &mut TokenStream, matcher: &Matcher, ctx: Context) {
	let Expr::Capture(cap) = &matcher.expr else { unreachable!() };
	let (capture, container) = match &cap.info {
		Some(info) => (info.resolved_type.clone(), info.container),
		_ => (quote! { () }, CapContainer::None),
	};

	chunk!(stream, {
		struct Matcher;
		impl ::gramex::Matcher<#{&matcher.matched_type}> for Matcher {
			type Capture<'a> = #capture;
			fn do_match<'a, M: ::gramex::Mode>(
				&self, matched: &'a #{&matcher.matched_type}, off: &mut usize,
			) -> MatchResult<Self::Capture<'a>, M> {
				let mut __cap__root = None;
				#{ctx.label}: {
					#do { gen_expr(stream, &cap.expr, ctx) }
				}?;
				M::ok(|| #do { gen_capture_unwrwap(stream, &cap.ident, container) })
			}
		}

		Matcher
	})
}
