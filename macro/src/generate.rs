use std::cell::Cell;

use chunked_quote::{chunk, quote};
use proc_macro2::{Ident, Literal, Punct, Spacing::Joint, Span, TokenStream};
use quote::ToTokens;

use crate::{
	capture::{CapChild, CapContainer, CapInfo, CapKind, pascal_case},
	cursor::ident,
	parse::{Atom, Capture, Expr, Matcher, Rep, Term},
};

#[derive(Debug, Clone, Copy)]
struct BlockLable<'a>(&'a Cell<u64>, u64);
impl ToTokens for BlockLable<'_> {
	fn to_tokens(&self, mut tokens: &mut TokenStream) {
		chunk!(tokens,
			#{Punct::new('\'', Joint)}
			#{ident!("mat_{}", self.1)}
		);
	}
}
impl BlockLable<'_> {
	fn next(&self) -> Self {
		self.0.set(self.0.get() + 1);
		BlockLable(self.0, self.0.get())
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
		match (self.capture, self.error) {
			(true, true) => chunk!(tokens, __M),
			(true, false) => chunk!(tokens, __M::WithoutError),
			(false, true) => chunk!(tokens, __M::WithoutCapture),
			(false, false) => chunk!(tokens, __Test),
		}
	}
}

#[derive(Debug, Copy, Clone)]
struct Context<'a> {
	label: BlockLable<'a>,
	mode: Mode,
}
impl Context<'_> {
	fn next_label(&self) -> Context {
		Context { label: self.label.next(), ..*self }
	}
	fn no_err(&self) -> Context {
		Context { mode: self.mode.no_err(), ..*self }
	}
}

fn fork<'a>(
	mut stream: &mut TokenStream, ctx: &'a Context<'a>,
	item: impl Fn(&mut TokenStream, Context<'a>),
) {
	let child_ctx = ctx.next_label();
	chunk!(stream, #{child_ctx.label}: {
		#do { item(stream, child_ctx) }
		#{child_ctx.mode}::ok(|| ())
	})
}

fn gen_expected_atom(stream: &mut TokenStream, atom: &Atom) {
	match atom {
		Atom::Any => chunk!(stream, __::EXPECTED_ANY),
		Atom::Matcher(matcher) => {
			chunk!(stream, (&#matcher).__expected())
		}
		Atom::Group(expr) => gen_expected(stream, expr),
		Atom::Call { .. } => chunk!(stream, ::gramex::Expected::None),
	}
}

#[rustfmt::skip]
fn gen_expected_or(stream: &mut TokenStream, exprs: &[Expr]) {
	chunk!(stream, ::gramex::Expected::OneOf(
		__::vec![
			#for expr in exprs #{
				#do { gen_expected(stream, expr) }.value(),
			}
		]
	));
}

fn gen_expected(stream: &mut TokenStream, expr: &Expr) {
	match expr {
		Expr::Unit { not: true, atom, .. } => chunk!(stream,
			__::expected_not(#do { gen_expected_atom(stream, atom) })
		),
		Expr::Unit { atom, .. } => gen_expected_atom(stream, atom),
		Expr::Range(left, right) => {
			chunk!(stream, (#left..=#right).__expected())
		}
		Expr::And(exprs) | Expr::Seq(exprs) => gen_expected(stream, &exprs[0]),
		Expr::Imply { cond, .. } => gen_expected(stream, cond),
		Expr::Capture(cap) => gen_expected(stream, &cap.expr),
		Expr::Error => {}
		Expr::Or(exprs) => gen_expected_or(stream, exprs),
	}
}

fn gen_atom_inline(mut stream: &mut TokenStream, atom: &Atom, ctx: Context) {
	match atom {
		Atom::Any => chunk!(stream,
			if __value.__skip_n(__off, 1) { #{ctx.mode}::ok(|| ()) }
			else { #{ctx.mode}::err(|| __::error_any(*__off)) }
		),
		Atom::Matcher(matcher) => chunk!(stream,
			(&#matcher).__do_match::<#{ctx.mode.no_cap()}>(&__value, __off)
		),
		Atom::Call { path, args } => chunk!(stream,
			#for part in path #{ #part }(
				#for arg in args #{ { #do { gen_matcher(stream, arg) } }, }
			).__do_match::<#{ctx.mode.no_cap()}>(&__value, __off)
		),
		Atom::Group(expr) => {
			fork(stream, &ctx, |stream, ctx| gen_expr(stream, expr, ctx))
		}
	}
}
fn gen_atom(mut stream: &mut TokenStream, atom: &Atom, ctx: Context) {
	match atom {
		Atom::Group(expr) => gen_expr(&mut stream, expr, ctx),
		_ => chunk!(stream,
			if let Err(err) = #do { gen_atom_inline(stream, atom, ctx) } {
			break #{ctx.label} Err(err)
		}),
	}
}

fn gen_rep_complex(
	stream: &mut TokenStream, rep: Rep, ctx: Context, do_fork: bool,
	item: impl Fn(&mut TokenStream, Context),
) {
	let Rep(start, end) = rep;
	chunk!(stream, {
		let mut __iter = 0;
		loop {
			let __start = *__off;
			if let Err(err) = #do {
				if do_fork { fork(stream, &ctx, item) }
				else { item(stream, ctx) }
			} {
				#if start != 0 #{
					if __iter < #{Literal::u32_unsuffixed(start)} { break #{ctx.label} Err(err) }
				}
				*__off = __start;
				break
			}
			__iter += 1;
			#if end != u32::MAX #{
				if __iter == #end { break }
			}
		}
	})
}

fn gen_rep(
	stream: &mut TokenStream, rep: Rep, ctx: Context, do_fork: bool,
	item: impl Fn(&mut TokenStream, Context),
) {
	if rep == Rep::ONCE {
		item(stream, ctx);
	} else if rep == Rep::OPTIONAL {
		chunk!(stream, {
			let __start = *__off;
			if #do {
				if do_fork { fork(stream, &ctx.no_err(), item) }
				else { item(stream, ctx) }
			}.is_err() { *__off = __start }
		})
	} else {
		gen_rep_complex(stream, rep, ctx, do_fork, item)
	}
}

fn gen_unit_near(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, rep, atom, .. } = expr else { unreachable!() };
	let gen_logic = |stream: &mut _, ctx| {
		gen_rep(stream, *rep, ctx, true, |stream, ctx| gen_atom(stream, atom, ctx))
	};
	chunk!(stream, {
		let __orig = &mut *__off;
		let __off = &mut __orig.clone();
		#if *not #{
			if #do { fork(stream, &ctx.no_err(), gen_logic) }.is_ok() {
				break #{ctx.label} #{ctx.mode}::err(|| __::error_not(
					#do { gen_expected_atom(stream, atom) }, true, *__orig
				))
			}
		}
		#else #{ #do { gen_logic(stream, ctx) } }
	})
}

fn gen_unit_not(stream: &mut TokenStream, atom: &Atom, rep: Rep, ctx: Context) {
	gen_rep(stream, rep, ctx, true, |stream, ctx| {
		chunk!(stream, {
			let __orig = &mut *__off;
			let __off = &mut __orig.clone();
			let __res = #do { gen_atom_inline(stream, atom, ctx.no_err()) };
			if __res.is_ok() || __res.is_err() && *__orig == __value.__len() {
				break #{ctx.label} #{ctx.mode}::err(|| __::error_not(
					#do { gen_expected_atom(stream, atom) }, __res.is_ok(), *__orig
				));
			}
			__value.__skip_n(__orig, 1);
		})
	});
}

fn gen_unit(stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	let Expr::Unit { not, near, rep, atom } = expr else { unreachable!() };
	if *not == false && *near == false && matches!(atom, Atom::Any) && rep.is_exact() {
		chunk!(stream,
			if !__value.__skip_n(__off, #{Literal::u32_unsuffixed(rep.0)}) {
				break #{ctx.label} #{ctx.mode}::err(|| __::error_any(*__off));
			};
		);
	} else if *near {
		gen_unit_near(stream, expr, ctx);
	} else if *not {
		gen_unit_not(stream, atom, *rep, ctx);
	} else if *rep != Rep::ONCE {
		gen_rep(stream, *rep, ctx, false, |stream, ctx| {
			gen_atom_inline(stream, atom, ctx)
		});
	} else {
		gen_atom(stream, atom, ctx);
	}
}

fn gen_or_branch(
	mut stream: &mut TokenStream, expr: &Expr, ind: usize,
	after: &impl Fn(&mut TokenStream, usize), new_ctx: Context,
) {
	chunk!(stream,
		if #do { gen_expr_inline(stream, expr, new_ctx.no_err()) }.is_ok() {
			#do { after(stream, ind) };
			break #{new_ctx.label} Ok::<_, ()>(());
		}
	)
}

fn gen_or(
	mut stream: &mut TokenStream, exprs: &[Expr], ctx: Context,
	before: impl Fn(&mut TokenStream, usize), after: impl Fn(&mut TokenStream, usize),
) {
	let new_ctx = ctx.next_label();
	chunk!(stream, _ = #{new_ctx.label}: {
		let __start = *__off;
		#for (ind, expr) in exprs.iter().enumerate() #{
			#do { before(stream, ind) }
			*__off = __start;
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
		*__off = __start;
		break #{ctx.label} #{ctx.mode}::err(|| {
			let __expected = #do { gen_expected_or(stream, exprs) };
			if *__off == __value.__len() {
				::gramex::MatchError::incomplete(__expected, __start)
			} else {
				::gramex::MatchError::mismatch(__expected, __start)
			}
		});
	};)
}

fn gen_and(stream: &mut TokenStream, exprs: &[Expr], ctx: Context) {
	chunk!(stream, {
		let __start = *__off;
		#do { gen_expr(stream, &exprs[0], ctx) }
		let __value = __value.__slice(0..*__off).unwrap();
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
		if #do { gen_expr_inline(stream, cond, ctx.no_err()) }.is_ok() {
			#do { gen_expr(stream, expr, ctx) }
		} else { *__off = __start }
	})
}

fn gen_atomic_capture(mut stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	match expr {
		Expr::Unit { atom: Atom::Matcher(matcher), .. } => chunk!(stream,
			(&#matcher).__do_match::<#{ctx.mode}>(&__value, __off)
		),
		Expr::Unit { atom: Atom::Call { path, args }, .. } => chunk!(stream,
			#for part in path #{ #part }(
				#for arg in args #{ { #do { gen_matcher(stream, arg) } }, }
			).do_match::<#{ctx.mode}>(&__value, __off)
		),
		Expr::Range(left, right) => chunk!(stream,
			(#left..=#right).__do_match::<#{ctx.mode}>(&__value, __off)
		),
		_ => unreachable!(),
	}
}

fn gen_capture_unwrwap(
	mut stream: &mut TokenStream, ident: &Ident, container: CapContainer,
) {
	chunk!(stream,
		#{ident!("__cap__{ident}")}
		#match container {
			CapContainer::None => #{ .unwrap() },
			CapContainer::Option => {},
			CapContainer::Vec => #{ .unwrap_or_else(|| __::Vec::new()) },
		}
	)
}

fn gen_capture_set(mut stream: &mut TokenStream, ident: &Ident, container: CapContainer) {
	chunk!(stream,
		#{ident!("__cap__{ident}")} #match container {
			CapContainer::None | CapContainer::Option => #{ = Some(__cap) },
			CapContainer::Vec => #{
				.get_or_insert_with(|| __::Vec::new()).push(__cap)
			},
		};
	)
}

fn gen_capture_normal(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: Context,
) {
	chunk!(stream, {
		let __start = *__off;
		#if matches!(info.kind, CapKind::Atomic { .. }) #{
			let __cap = #do { gen_atomic_capture(stream, &cap.expr, ctx) };
			if let Err(err) = __cap { break #{ctx.label} Err(err) }
		} #else #{
			#do { gen_expr(stream, &cap.expr, ctx) }
		}
		if #{ctx.mode}::DO_CAPTURE {
			let __cap = #match &info.kind {
				CapKind::Atomic { .. } => #{
					#{ctx.mode}::unwrap_success(__::unwrap_result(__cap))
				},
				CapKind::Normal { .. } => #{ __value.__slice(__start..*__off).unwrap() },
				CapKind::UnitStruct => #{
					#{&info.resolved_type}(__value.__slice(__start..*__off).unwrap())
				},
				_ => unreachable!(),
			};
			#if matches!(&info.kind,
				CapKind::Atomic { need_from: true } | CapKind::Normal { need_from: true }
			) #{
				let __cap = __::Into::<#{&info.resolved_type}>::into(__cap);
			}
			#if let Some(map) = &cap.map #{
				let #{&cap.ident} = __cap;
				let __cap = #map;
			}
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		}
	});
}

fn gen_capture_fielded(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: Context,
) {
	let fields = match &info.kind {
		CapKind::Struct { fields, .. } => fields,
		CapKind::Tuple(fields) => fields,
		CapKind::ReduceMap(fields) => fields,
		_ => unreachable!(),
	};

	chunk!(stream, {
		let __start = *__off;
		#for CapChild { name, ..} in fields #{
			let mut #{ident!("__cap__{name}")} = None;
		}
		#do { gen_expr(stream, &cap.expr, ctx) }
		if #{ctx.mode}::DO_CAPTURE {
			#match &info.kind {
				CapKind::ReduceMap(_) => #{
					#for field in fields #{
						let #{&field.name} = #do {
							gen_capture_unwrwap(stream, &field.name, field.container)
						};
					}
					let __cap = #{&cap.map};
				},
				CapKind::Tuple(_) => #{
					let __cap = (#for field in fields #{
						#do { gen_capture_unwrwap(stream, &field.name, field.container) },
					});
				}
				CapKind::Struct { is_generated, .. } => #{
					let __cap = #{&info.resolved_type} {
						#for field in fields #{
							#{&field.name}: #do {
								gen_capture_unwrwap(stream, &field.name, field.container)
							},
						}
						#if *is_generated #{
							__life_marker: __::PhantomData,
						}
					};
				}
				_ => unreachable!()
			}
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		}
	});
}

fn gen_capture_enum(
	stream: &mut TokenStream, cap: &Capture, vars: &[Option<CapChild>], info: &CapInfo,
	ctx: Context,
) {
	let Expr::Or(exprs) = &cap.expr else { unreachable!() };
	let before = |mut stream: &mut TokenStream, ind| {
		chunk!(stream,
			#if let Some(CapChild { name, .. }) = &vars[ind] #{
				let mut #{ident!("__cap__{name}")} = None;
			}
		)
	};
	let after = |mut stream: &mut TokenStream, ind| {
		chunk!(stream, if #{ctx.mode}::DO_CAPTURE {
			let __cap = #{&info.resolved_type}::
			#match &vars[ind] {
				Some(CapChild { name, container, .. }) => #{ #
					{pascal_case(name)} (#do {
						gen_capture_unwrwap(stream, name, *container)
					})
				},
				_ => #{None},
			};
			#do { gen_capture_set(stream, &cap.ident, info.container) }
		})
	};
	gen_or(stream, exprs, ctx, before, after);
}

fn gen_capture(stream: &mut TokenStream, cap: &Capture, ctx: Context) {
	let Some(info) = &cap.info else { return gen_expr(stream, &cap.expr, ctx) };

	gen_rep(stream, cap.rep, ctx, true, |stream, ctx| match &info.kind {
		CapKind::Atomic { .. } | CapKind::Normal { .. } | CapKind::UnitStruct => {
			gen_capture_normal(stream, cap, info, ctx)
		}
		CapKind::Struct { .. } | CapKind::Tuple(_) | CapKind::ReduceMap(_) => {
			gen_capture_fielded(stream, cap, info, ctx)
		}
		CapKind::Enum(vars) => gen_capture_enum(stream, cap, vars, info, ctx),
	});
}

fn gen_expr_inline(mut stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	match expr {
		Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom }
			if !matches!(atom, Atom::Group(_)) =>
		{
			gen_atom_inline(stream, atom, ctx)
		}
		Expr::Range(left, right) => chunk!(stream,
			(#left..=#right).__do_match::<#{ctx.mode.no_cap()}>(&__value, __off)
		),
		_ => fork(stream, &ctx, |stream, ctx| gen_expr(stream, expr, ctx)),
	}
}

fn gen_expr(mut stream: &mut TokenStream, expr: &Expr, ctx: Context) {
	match expr {
		Expr::Unit { .. } => gen_unit(stream, expr, ctx),
		Expr::Range(left, right) => chunk!(stream,
			if let Err(err) =
				(#left..=#right).__do_match::<#{ctx.mode.no_cap()}>(&__value, __off)
			{ break #{ctx.label} Err(err) }
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

fn gen_match_root(mut stream: &mut TokenStream, expr: &Expr, capture: bool) {
	let count = Cell::new(0);
	let ctx = Context {
		label: BlockLable(&count, 0),
		mode: Mode { capture: true, error: true },
	};
	chunk!(stream, use ::gramex::{
		__private as __, MatchAble as _, Mode as _, Matcher as _,
		modes::Test as __Test, #do{}
	};);
	if capture {
		let Expr::Capture(cap) = &expr else { unreachable!() };
		let container =
			cap.info.as_ref().map_or(CapContainer::None, |info| info.container);

		chunk!(stream,
			let mut #{ident!("__cap__{}", cap.ident)} = None;
			let mut res = match #do { gen_expr_inline(stream, expr, ctx) } {
				Ok(_) => __M::ok(|| #do { gen_capture_unwrwap(stream, &cap.ident, container) }),
				Err(err) => Err(err),
			};
		)
	} else {
		chunk!(stream,let mut res = #do { gen_expr_inline(stream, expr, ctx) };);
	}
}

fn gen_matcher_impl(
	mut stream: &mut TokenStream, matcher_ident: &Ident, expr: &Expr,
	matched_type: &TokenStream, args: &[&Ident],
	prologue: impl Fn(&mut TokenStream, bool),
) {
	let Expr::Capture(cap) = &expr else { unreachable!() };
	let capture =
		cap.info.as_ref().map_or(TokenStream::new(), |i| i.resolved_type.clone());

	chunk!(stream,
		# #[allow(nonstandard_style, unused_imports, )]
		impl<#for arg in args #{
			#arg: ::gramex::Matcher<#matched_type>,
		}> ::gramex::Matcher<#matched_type> for #matcher_ident<
			#for arg in args #{ #arg }
		> {
			type Capture<'src> = #capture;
			fn do_match<'src, __M: ::gramex::Mode>(
				&self, __value: &'src #{&matched_type}, __off: &mut usize,
			) -> ::gramex::MatchResult<Self::Capture<'src>, __M> {
				#do { prologue(stream, true) }
				#do { gen_match_root(stream, expr, true) }
				return res;
			}
			fn expected(&self) -> ::gramex::Expected {
				use ::gramex::{ Matcher as _, __private as __ };
				#do { prologue(stream, false) }
				#do { gen_expected(stream, &cap.expr) }
			}
		}
	);
}

pub fn gen_term(mut stream: &mut TokenStream, term: &Term, matched_type: &TokenStream) {
	let args = term.args.iter().filter(|arg| *arg != "_").collect::<Vec<_>>();
	let args_t = args.iter().map(|i| pascal_case(i)).collect::<Vec<_>>();
	let matcher_ident = ident!("{}__Matcher", span = term.name.span(), term.name);
	let Expr::Capture(cap) = &term.expr else { unreachable!() };

	chunk!(stream,
		#if args.is_empty() #{
			pub use #matcher_ident as #{&term.name};
		} #else #{
			pub fn #{&term.name}<#for arg in &args_t #{
				#arg: ::gramex::Matcher<#matched_type>,
			}>(#for arg in &args #{ #arg: #{pascal_case(arg)} })
				-> #matcher_ident<#for arg in &args_t #{ #arg, }>
			{
				#matcher_ident(#for arg in &args #{ #arg, })
			}
		}

		# #[doc(hidden)]
		# #[allow(nonstandard_style)]
		pub struct #matcher_ident
		#if args.len() > 0 #{
			<#for arg in &args #{
				#arg: ::gramex::Matcher<#matched_type>,
			}>
			(#for arg in &args #{ #arg })
		};
		#do { gen_matcher_impl(stream, &matcher_ident, &term.expr, matched_type, &args,
			|mut stream, in_matcher| chunk!(stream,
				#if args.len() > 0 #{
					let Self(#for arg in &args #{ #arg, }) = self;
				}
				#if cap.map.is_some() && in_matcher #{
					fn #{&term.name} () {}
				}
			)
		) }
	);
}

pub fn gen_matcher(mut stream: &mut TokenStream, matcher: &Matcher) {
	let matched_type = matcher.matched_type.as_ref().unwrap();
	chunk!(stream,
		struct Matcher;
		#do {
			gen_matcher_impl(stream, &ident!("Matcher"), &matcher.expr, matched_type, &[], |_, _| ());
		}
		Matcher
	)
}

pub fn gen_match_expr(
	mut stream: &mut TokenStream, capture: bool, error: bool, value: TokenStream,
	expr: &Expr,
) {
	chunk!(stream,
		let __value = #value;
		let __off = &mut 0;
		type __M = #match (capture, error) {
			(false, false) => #{ ::gramex::modes::Test },
			(false, true) => #{ ::gramex::modes::Check },
			(true, false) => #{ ::gramex::modes::Capture },
			(true, true) => #{ ::gramex::modes::Parse },
		};
		#do { gen_match_root(stream, expr, capture) }
		if res.is_ok() && *__off != __value.__len() {
			res = __M::err(|| ::gramex::MatchError::excess(*__off));
		}
		::gramex::IntoResult::into_result::<__M>(res)
	)
}
