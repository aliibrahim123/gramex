//! codegen house, transform ast into [`TokenStream`]

// matching is done inside matcher struct / inside inlined expression, using nested labeled blocks that endless branching and primitive offset logic
// everything is generalized over matching mode using guards and selectors.
// `__off`: current offset, `__value`: matched value, `__cap__ident`: a capture, `__`: `gramex::__private`, `__T`: `gramex::*::T` to avoid collisions.
// will generate a tone of code that is highly optimizable by rustc, but is still enourmess
// hardwrite your matchers if you dont like the generated code, or use cursors

use std::cell::Cell;

use chunked_quote::chunk;
use proc_macro2::{Ident, Literal, Punct, Spacing::Joint, Span, TokenStream};
use quote::{ToTokens, quote};

use crate::{
	capture::{CapChild, CapContainer, CapInfo, CapKind, pascal_case},
	cursor::ident,
	parse::{Atom, Capture, EnumMatcher, Expr, Matcher, Rep, Term, Variant},
};

/// unique inside a matcher
#[derive(Debug, Clone, Copy)]
struct BlockLable<'a> {
	counter: &'a Cell<u64>,
	id: u64,
}
impl ToTokens for BlockLable<'_> {
	fn to_tokens(&self, mut tokens: &mut TokenStream) {
		chunk!(tokens,
			#{Punct::new('\'', Joint)}
			#{ident!("mat_{}", self.id)}
		);
	}
}
impl BlockLable<'_> {
	fn next(&self) -> Self {
		self.counter.set(self.counter.get() + 1);
		BlockLable { counter: self.counter, id: self.counter.get() }
	}
}

/// a `gramex::Mode` type
#[derive(Debug, Clone, Copy)]
struct Mode {
	/// is concrete (like `Test`) or a type parameer `M`
	is_concrete: bool,
	capture: bool,
	error: bool,
}
impl Mode {
	/// a type parameter `M`
	fn param() -> Mode {
		Mode { is_concrete: false, capture: true, error: true }
	}
	fn no_cap(self) -> Mode {
		Mode { capture: false, ..self }
	}
	fn no_err(self) -> Mode {
		Mode { error: false, ..self }
	}
}
impl ToTokens for Mode {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match (self.is_concrete, self.capture, self.error) {
			(false, true, true) => chunk!(tokens, __M),
			(false, true, false) => chunk!(tokens, __M::WithoutError),
			(false, false, true) => chunk!(tokens, __M::WithoutCapture),
			(_, false, false) => chunk!(tokens, __Test),
			(true, true, true) => chunk!(tokens, __Parse),
			(true, true, false) => chunk!(tokens, __Capture),
			(true, false, true) => chunk!(tokens, __Check),
		}
	}
}

/// state for a given match unit
#[derive(Debug, Copy, Clone)]
struct Context<'a> {
	/// label of the outer block
	label: BlockLable<'a>,
	mode: Mode,
	/// used inside `Expected` generation, `0` stop recursion
	expected_fuel: u8,
	matched_type: Option<&'a TokenStream>,
}
impl Context<'_> {
	fn new<'a>(
		block_counter: &'a Cell<u64>, mode: Mode, matched_type: Option<&'a TokenStream>,
	) -> Context<'a> {
		Context {
			label: BlockLable { counter: block_counter, id: 0 },
			mode,
			expected_fuel: DEFAULT_EXPECTED_FUEL,
			matched_type,
		}
	}
	fn next_label(&self) -> Context<'_> {
		Context { label: self.label.next(), ..*self }
	}
	fn no_err(&self) -> Context<'_> {
		Context { mode: self.mode.no_err(), ..*self }
	}
}

/// create a child block with a new label, of type `Result<(), MatchError>`
fn fork<'a>(
	mut stream: &mut TokenStream, ctx: &'a Context<'a>,
	inner: impl Fn(&mut TokenStream, &Context<'a>),
) {
	let child_ctx = ctx.next_label();
	chunk!(stream, #{child_ctx.label}: {
		#do { inner(stream, &child_ctx) }
		__::ok_unit::<#{child_ctx.mode}>()
	});
}

// `Expected` generation is used in or, not and `Matcher::expected`
// it is recursive, for non conditionals, support expressive Expected trees but it is a best guest

// good for `!('a' | 'b' | 'c')` and anything below
const DEFAULT_EXPECTED_FUEL: u8 = 3;

/// generate expression evaluating to `Expected` for an [`Atom`]
fn gen_expected_atom(
	mut stream: &mut TokenStream, atom: &Atom, fuel: u8,
	matched_type: Option<&TokenStream>,
) {
	// a type may impl more that 1 `Matcher`, so `matched_type` is necessary for disambiguation
	if matched_type.is_none() {
		return chunk!(stream, __::Expected::None);
	}
	match atom {
		Atom::Any => chunk!(stream, __::Expected::SomeThing),
		Atom::Matcher(matcher) => {
			chunk!(stream, <_ as __Matcher<#{matched_type}>>::expected(&#matcher));
		}
		Atom::Group(expr) => {
			let Some(expr) = locate_expected(expr) else {
				return chunk!(stream, __::Expected::None);
			};
			gen_expected(stream, expr, fuel, matched_type);
		}
		Atom::Call { .. } => chunk!(stream, __::Expected::None),
	}
}

/// generate expression evaluating to `Expected` for [`Expr::Or`]
fn gen_expected_or(
	stream: &mut TokenStream, exprs: &[Expr], fuel: u8,
	matched_type: Option<&TokenStream>,
) {
	let exprs = exprs.iter().filter_map(locate_expected).collect::<Vec<_>>();
	if exprs.is_empty() {
		return chunk!(stream, __::Expected::None);
	}
	chunk!(stream, __::expected_or(
		&[#for expr in exprs #{
			#do { gen_expected(stream, expr, fuel - 1, matched_type) },
		}]
	));
}

/// walk the ast to find the first expression able to generate an `Expected`
fn locate_expected(expr: &Expr) -> Option<&Expr> {
	match expr {
		// 0..n repetition is conditional
		Expr::Unit { rep, .. } if rep.0 == 0 => None,
		Expr::Unit { not: false, atom: Atom::Group(expr), .. } => locate_expected(expr),
		Expr::And(exprs) => locate_expected(&exprs[0]),
		Expr::Seq(exprs) => exprs.iter().find_map(locate_expected),
		Expr::Capture(cap) => locate_expected(&cap.expr),
		Expr::Error | Expr::Imply { .. } => None,
		Expr::Unit { .. } | Expr::Or(_) => Some(expr),
	}
}

/// generate expression evaluating to `Expected` for any [`Expr`]
fn gen_expected(
	stream: &mut TokenStream, expr: &Expr, fuel: u8, matched_type: Option<&TokenStream>,
) {
	if fuel == 0 || matched_type.is_none() {
		// safe default
		chunk!(stream, __::Expected::None);
		return;
	}
	match expr {
		Expr::Unit { not: true, atom, .. } if fuel > 1 => chunk!(stream,
			__::expected_not(&#do { gen_expected_atom(stream, atom, fuel - 1, matched_type) })
		),
		Expr::Unit { not: true, .. } => chunk!(stream, __::Expected::None),
		Expr::Unit { atom, .. } => gen_expected_atom(stream, atom, fuel, matched_type),
		Expr::Or(exprs) if fuel > 1 => gen_expected_or(stream, exprs, fuel, matched_type),
		Expr::Or(_) => chunk!(stream, __::Expected::None),
		_ => unreachable!(),
	}
}

/// generate expression evaluating to `MatchError` for a `!atom`
fn gen_error_not(
	mut stream: &mut TokenStream, atom: &Atom, is_incomplete: impl ToTokens,
	ctx: &Context,
) {
	if ctx.mode.error {
		chunk!(stream, break #{ctx.label} #{ctx.mode}::err(|| __::MatchError::expected(
			__::expected_not(&#do {
				gen_expected_atom(stream, atom,  ctx.expected_fuel - 1, ctx.matched_type);
			}),
			#is_incomplete, *__orig
		)));
	} else {
		chunk!(stream, break #{ctx.label} Err(()); );
	}
}
/// generate expression evaluating to `MatchError` for a [`Expr::Or`]
fn gen_error_or(mut stream: &mut TokenStream, exprs: &[Expr], ctx: &Context) {
	if ctx.mode.error {
		chunk!(stream, break #{ctx.label} #{ctx.mode}::err( || __::MatchError::expected(
			#do { gen_expected_or(stream, exprs, ctx.expected_fuel - 1, ctx.matched_type) },
			*__off == <_ as __MatchAble>::len(__value), __start
		)));
	} else {
		chunk!(stream, break #{ctx.label} Err(()); );
	}
}
/// generate expression evaluating to `Matcher` for a [`Atom::Call`] argument
fn gen_call_matcher(stream: &mut TokenStream, arg: &Matcher) {
	// skip matcher bolerpliat for bare atoms, as they resolve directly to `Matcher`
	if let Expr::Unit { atom, not: false, near: false, rep: Rep::ONCE } = &arg.cap.expr
		&& let Atom::Matcher(matcher) = atom
	{
		matcher.to_tokens(stream);
	} else {
		gen_matcher(stream, arg);
	}
}

/// generate matching expression evaluating to `MatchResult<()>` from atom
fn gen_atom_inline(mut stream: &mut TokenStream, atom: &Atom, ctx: &Context) {
	// skip capture matching feature for optimization
	match atom {
		// skip_n(1)
		Atom::Any => chunk!(stream,
			<_ as __MatchAble>::skip_n::<#{ctx.mode.no_cap()}>(__value, __off, 1)
		),
		// matcher.do_match()
		Atom::Matcher(matcher) => chunk!(stream,
			<_ as __Matcher<_>>::do_match::<#{ctx.mode.no_cap()}>(&#matcher, __value, __off)
		),
		// path(..args).do_match()
		Atom::Call { path, args } => chunk!(stream,
			<_ as __Matcher<_>>::do_match::<#{ctx.mode.no_cap()}>(
				&#for part in path #{ #part }(
					#for arg in args #{{ #do { gen_call_matcher(stream, arg) } }, }
				),
				__value, __off
			)
		),
		Atom::Group(expr) => gen_expr_inline(stream, expr, ctx),
	}
}
/// append matching logic for [`Atom`]
fn gen_atom(mut stream: &mut TokenStream, atom: &Atom, ctx: &Context) {
	if let Atom::Group(expr) = atom {
		// reduce uneccessary semantic scafolding
		gen_expr(stream, expr, ctx);
	} else {
		chunk!(stream,
			if let Err(err) = #do { gen_atom_inline(stream, atom, ctx) } {
			break #{ctx.label} Err(err)
		});
	}
}

/// append matching logic for non simple `[n..m]` [`Rep`]
fn gen_rep_complex(
	stream: &mut TokenStream, rep: Rep, ctx: &Context, do_fork: bool,
	item: impl Fn(&mut TokenStream, &Context),
) {
	let Rep(start, end) = rep;
	//              min            max
	// ok:  continue | continue     |  break
	// err: fail     | atomic-break |
	chunk!(stream, {
		let mut __iter = 0;
		loop {
			let __start = *__off;
			if let Err(err) = #do {
				if do_fork { fork(stream, ctx, item) }
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
	});
}

/// append matching logic for any [`Rep`]
fn gen_rep(
	stream: &mut TokenStream, rep: Rep, ctx: &Context, do_fork: bool,
	item: impl Fn(&mut TokenStream, &Context),
) {
	if rep == Rep::ONCE {
		item(stream, ctx);
	} else if rep == Rep::OPTIONAL {
		// atomic match
		chunk!(stream, {
			let __start = *__off;
			if #do {
				if do_fork { fork(stream, &ctx.no_err(), item) }
				else { item(stream, ctx) }
			}.is_err() { *__off = __start }
		});
	} else {
		gen_rep_complex(stream, rep, ctx, do_fork, item);
	}
}

/// append matching logic for near modified [`Expr::Unit`]
fn gen_unit_near(stream: &mut TokenStream, expr: &Expr, ctx: &Context) {
	let Expr::Unit { not, rep, atom, .. } = expr else { unreachable!() };
	let gen_logic = |stream: &mut _, ctx: &_| {
		gen_rep(stream, *rep, ctx, true, |stream, ctx| gen_atom(stream, atom, ctx));
	};
	chunk!(stream, {
		// fork offset
		let __orig = &mut *__off;
		let __off = &mut __orig.clone();

		#if *not #{
			if #do { fork(stream, &ctx.no_err(), gen_logic) }.is_ok() {
				#do { gen_error_not(stream, atom, quote! { false }, ctx) }
			}
		}
		#else #{ #do { gen_logic(stream, ctx) } }
	});
}

/// append matching logic for not modified [`Expr::Unit`]
fn gen_unit_not(stream: &mut TokenStream, atom: &Atom, rep: Rep, ctx: &Context) {
	gen_rep(stream, rep, ctx, true, |stream, ctx| {
		chunk!(stream, {
			// fork offset
			let __orig = &mut *__off;
			let __off = &mut __orig.clone();

			let __res = #do { gen_atom_inline(stream, atom, &ctx.no_err()) };
			// only non incomplete errors succed
			if __res.is_ok() || __res.is_err() && *__orig == <_ as __MatchAble>::len(__value) {
				#do { gen_error_not(stream, atom, quote! { !__res.is_ok() }, ctx) }
			}
			// skip 1 token
			_ = <_ as __MatchAble>::skip_n::<#{ctx.mode.no_cap().no_err()}>(__value, __orig, 1);
		});
	});
}

/// append matching logic for [`Expr::Unit`]
fn gen_unit(mut stream: &mut TokenStream, expr: &Expr, ctx: &Context) {
	let Expr::Unit { not, near, rep, atom } = expr else { unreachable!() };
	// optimization for `_[n]` though `skip_n(n)`
	if !*not && !*near && matches!(atom, Atom::Any) && rep.is_exact() {
		chunk!(stream,
			if let Err(err) = <_ as __MatchAble>::skip_n::<#{ctx.mode.no_cap()}>(
				__value, __off, #{Literal::u32_unsuffixed(rep.0)}
			) { break #{ctx.label} Err(err) }
		);
	} else if *near {
		gen_unit_near(stream, expr, ctx);
	} else if *not {
		gen_unit_not(stream, atom, *rep, ctx);
	} else if *rep != Rep::ONCE {
		gen_rep(stream, *rep, ctx, false, |stream, ctx| {
			gen_atom_inline(stream, atom, ctx);
		});
	} else {
		gen_atom(stream, atom, ctx);
	}
}

/// append matching logic for an [`Expr::Or`] branch
fn gen_or_branch(
	mut stream: &mut TokenStream, expr: &Expr, ind: usize,
	before: impl Fn(&mut TokenStream, usize), after: &impl Fn(&mut TokenStream, usize),
	or_ctx: &Context, parent_ctx: &Context,
) {
	chunk!(stream,
		#do { before(stream, ind) }
		// specilization of `Expr::Imply` in `Expr::Or`
		#if let Expr::Imply { cond, expr } = expr #{
			if #do { gen_expr_inline(stream, cond, &or_ctx.no_err()) }.is_ok() {
				// target parent ctx on errors breaks
				#do { gen_expr(stream, expr, parent_ctx) };
				#do { after(stream, ind) };
				break #{or_ctx.label} Ok::<_, ()>(());
			}
		}
		#else #{
			if #do { gen_expr_inline(stream, expr, &or_ctx.no_err()) }.is_ok() {
				#do { after(stream, ind) };
				break #{or_ctx.label} Ok::<_, ()>(());
			}
		}
		*__off = __start;
	);
}

/// append matching logic for [`Expr::Or`]
fn gen_or(
	mut stream: &mut TokenStream, exprs: &[Expr], ctx: &Context,
	before: impl Fn(&mut TokenStream, usize), after: impl Fn(&mut TokenStream, usize),
) {
	let new_ctx = &ctx.next_label();
	chunk!(stream, _ = #{new_ctx.label}: {
		let __start = *__off;
		// series of: if match(expr) { break }; off = start
		#do { for (ind, expr) in exprs.iter().enumerate() {
			gen_or_branch(stream, expr, ind, &before, &after, new_ctx, ctx) ;
		} }
		#do { gen_error_or(stream, exprs, ctx) }
	};);
}

/// append matching logic for [`Expr::And`]
fn gen_and(stream: &mut TokenStream, exprs: &[Expr], ctx: &Context) {
	chunk!(stream, {
		// match and slice
		let __start = *__off;
		#do { gen_expr(stream, &exprs[0], ctx) }
		let __value = <_ as __MatchAble>::slice(__value, 0..*__off).unwrap();
		let __value = __value.__as_matchable();

		// fork and match
		let __off = &mut 0;
		#for expr in &exprs[1..] #{
			*__off = __start;
			#do { gen_expr(stream, expr, ctx) }
		}
	});
}

/// append matching logic for [`Expr::Imply`]
fn gen_imply(stream: &mut TokenStream, cond: &Expr, expr: &Expr, ctx: &Context) {
	chunk!(stream, {
		let __start = *__off;
		if #do { gen_expr_inline(stream, cond, &ctx.no_err()) }.is_ok() {
			#do { gen_expr(stream, expr, ctx) }
		} else { *__off = __start }
	});
}

/// generate matching expression evaluating to `MatchResult<Capture>` from [`Expr::Unit`]
fn gen_atomic_capture(mut stream: &mut TokenStream, expr: &Expr, ctx: &Context) {
	match expr {
		// matcher.do_match()
		Expr::Unit { atom: Atom::Matcher(matcher), .. } => chunk!(stream,
			<_ as __Matcher<_>>::do_match::<#{ctx.mode}>(&#matcher, __value, __off)
		),
		// path(..args).do_match()
		Expr::Unit { atom: Atom::Call { path, args }, .. } => chunk!(stream,
			<_ as __Matcher<_>>::do_match::<#{ctx.mode}>(
				&#for part in path #{ #part }(
					#for arg in args #{{ #do { gen_call_matcher(stream, arg) } }, }
				),
				__value, __off
			)
		),
		_ => unreachable!(),
	}
}

/// generate unwraping logic for capture based on its conatiner
fn gen_cap_unwrwap(mut stream: &mut TokenStream, ident: &Ident, container: CapContainer) {
	chunk!(stream,
		#{ident!("__cap__{ident}")}
		#match container {
			CapContainer::None => #{ .unwrap() },
			CapContainer::Option => {},
			CapContainer::Vec => #{ .unwrap_or_else(|| __::Vec::new()) },
		}
	);
}

/// generate setting logic for capture based on its conatiner
fn gen_cap_set(mut stream: &mut TokenStream, ident: &Ident, container: CapContainer) {
	chunk!(stream,
		#{ident!("__cap__{ident}")} #match container {
			CapContainer::None | CapContainer::Option => #{ = Some(__cap) },
			CapContainer::Vec => #{
				.get_or_insert_with(|| __::Vec::new()).push(__cap)
			},
		};
	);
}

/// generate expression producing a `Atomic` / `Slice` / `UnitStruct` capture
fn gen_cap_normal_produce(mut stream: &mut TokenStream, info: &CapInfo, ctx: &Context) {
	match &info.kind {
		// cap.unwrap()
		CapKind::Atomic { .. } => chunk!(stream,
			#{ctx.mode}::unwrap_success(__::unwrap_result(__cap))
		),
		// value.slice()
		CapKind::Slice { .. } => {
			chunk!(stream, <_ as __MatchAble>::slice(__value, __start..*__off).unwrap());
		}
		// Struct(value.slice())
		CapKind::UnitStruct => chunk!(stream, #{&info.resolved_type}(
			<_ as __MatchAble>::slice(__value, __start..*__off).unwrap()
		)),
		_ => unreachable!(),
	}
}

/// append matching logic for `Atomic` / `Slice` / `UnitStruct` [`Capture`]
fn gen_cap_normal(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: &Context,
) {
	chunk!(stream, {
		let __start = *__off;

		// match
		#if matches!(info.kind, CapKind::Atomic { .. }) #{
			let __cap = #do { gen_atomic_capture(stream, &cap.expr, ctx) };
			if let Err(err) = __cap { break #{ctx.label} Err(err) }
		} #else #{
			#do { gen_expr(stream, &cap.expr, ctx) }
		}

		#if !ctx.mode.is_concrete #{ if #{ctx.mode}::DO_CAPTURE } {
			let __cap = #do { gen_cap_normal_produce(stream, info, ctx) };

			// transform
			#if matches!(&info.kind,
				CapKind::Atomic { need_from: true } | CapKind::Slice { need_from: true }
			) #{
				let __cap = __::Into::<#{&info.resolved_type}>::into(__cap);
			}
			#if let Some(map) = &cap.map #{
				let #{&cap.ident} = __cap;
				let __cap = #map;
			}

			#do { gen_cap_set(stream, &cap.ident, info.container) }
		}
	});
}

/// generate expression producing a `Struct` / `Tuple` / `ReduceMap` capture
fn gen_cap_fielded_produce(
	mut stream: &mut TokenStream, info: &CapInfo, map: Option<&TokenStream>,
) {
	match &info.kind {
		CapKind::ReduceMap(fields) => chunk!(stream,
			#for field in fields #{
				let #{&field.name} = #do {
					gen_cap_unwrwap(stream, &field.name, field.container);
				};
			}
			let __cap = #{map};
		),
		CapKind::Tuple(fields) => chunk!(stream,
			let __cap = (#for field in fields #{
				#do { gen_cap_unwrwap(stream, &field.name, field.container) },
			});
		),
		CapKind::Struct { is_generated, fields } => chunk!(stream,
			let __cap = #{&info.resolved_type} {
				#for field in fields #{
					#{&field.name}: #do {
						gen_cap_unwrwap(stream, &field.name, field.container);
					},
				}
				#if *is_generated #{
					__life_marker: __::PhantomData,
				}
			};
		),
		_ => unreachable!(),
	}
}

/// append matching logic for `Struct` / `Tuple` / `ReduceMap` [`Capture`]
fn gen_cap_fielded(
	stream: &mut TokenStream, cap: &Capture, info: &CapInfo, ctx: &Context,
) {
	let (CapKind::Struct { fields, .. }
	| CapKind::Tuple(fields)
	| CapKind::ReduceMap(fields)) = &info.kind
	else {
		unreachable!()
	};

	chunk!(stream, {
		let __start = *__off;
		//
		#for CapChild { name, ..} in fields #{
			let mut #{ident!("__cap__{name}")} = None;
		}
		#do { gen_expr(stream, &cap.expr, ctx) }
		#if !ctx.mode.is_concrete #{ if #{ctx.mode}::DO_CAPTURE } {
			#do { gen_cap_fielded_produce(stream, info, cap.map.as_ref()) }
			#do { gen_cap_set(stream, &cap.ident, info.container); }
		}
	});
}

/// append matching logic for `Enum` [`Capture`]
fn gen_cap_enum(
	stream: &mut TokenStream, cap: &Capture, vars: &[Option<CapChild>], info: &CapInfo,
	ctx: &Context,
) {
	// reuse or logic such that each branch become:
	// let var_cap = None;
	// if match { cap = Enum::Var(var_cap.unwrap()); break }

	let Expr::Or(exprs) = &cap.expr else { unreachable!() };
	let before = |mut stream: &mut TokenStream, ind| {
		chunk!(stream,
			#if let Some(CapChild { name, .. }) = &vars[ind] #{
				let mut #{ident!("__cap__{name}")} = None;
			}
		);
	};

	let after = |mut stream: &mut TokenStream, ind| {
		chunk!(stream,
			#if !ctx.mode.is_concrete #{ if #{ctx.mode}::DO_CAPTURE } {
				let __cap = #{&info.resolved_type}::
				#match &vars[ind] {
					Some(CapChild { name, container, .. }) => #{
						#{pascal_case(name)}
						(#do { gen_cap_unwrwap(stream, name, *container) })
					},
					_ => #{ None },
				};
				#do { gen_cap_set(stream, &cap.ident, info.container) }
			}
		);
	};
	gen_or(stream, exprs, ctx, before, after);
}

/// append matching logic for [`Capture`]
fn gen_cap(stream: &mut TokenStream, cap: &Capture, ctx: &Context) {
	// if symantic analysis failed, fallback to inner expresssion
	let Some(info) = &cap.info else { return gen_expr(stream, &cap.expr, ctx) };

	gen_rep(stream, cap.rep, ctx, true, |stream, ctx| match &info.kind {
		CapKind::Atomic { .. } | CapKind::Slice { .. } | CapKind::UnitStruct => {
			gen_cap_normal(stream, cap, info, ctx);
		}
		CapKind::Struct { .. } | CapKind::Tuple(_) | CapKind::ReduceMap(_) => {
			gen_cap_fielded(stream, cap, info, ctx);
		}
		CapKind::Enum(vars) => gen_cap_enum(stream, cap, vars, info, ctx),
	});
}

/// append matching logic for [`Expr`]
fn gen_expr(stream: &mut TokenStream, expr: &Expr, ctx: &Context) {
	match expr {
		Expr::Unit { .. } => gen_unit(stream, expr, ctx),
		Expr::Imply { cond, expr } => gen_imply(stream, cond, expr, ctx),
		Expr::Seq(exprs) => {
			for expr in exprs {
				gen_expr(stream, expr, ctx);
			}
		}
		Expr::And(exprs) => gen_and(stream, exprs, ctx),
		Expr::Or(exprs) => gen_or(stream, exprs, ctx, |_, _| (), |_, _| ()),
		Expr::Capture(cap) => gen_cap(stream, cap, ctx),
		Expr::Error => {}
	}
}
/// generate matching expression evaluating to `MatchResult<()>` for a [`Expr`]
fn gen_expr_inline(stream: &mut TokenStream, expr: &Expr, ctx: &Context) {
	match expr {
		Expr::Unit { not: false, near: false, rep: Rep::ONCE, atom }
			if !matches!(atom, Atom::Group(_)) =>
		{
			gen_atom_inline(stream, atom, ctx);
		}
		_ => fork(stream, ctx, |stream, ctx| gen_expr(stream, expr, ctx)),
	}
}
/// generate matching expression evaluating to `MatchResult<()>` for a [`Capture`]
fn gen_cap_inline(stream: &mut TokenStream, cap: &Capture, ctx: &Context) {
	fork(stream, ctx, |stream, ctx| gen_cap(stream, cap, ctx));
}

/// generate common imports
pub fn gen_imports(stream: &mut TokenStream) {
	chunk!(stream, use ::gramex::{ #do{}
		__private as __, MatchAble as __MatchAble, Mode as __Mode, Matcher as __Matcher,
		modes::Test as __Test, __private::AsMatchAble as _
	}; );
}

/// generate matching logic for root [`Capture`], save result in `__res`
fn gen_root_cap(
	mut stream: &mut TokenStream, cap: &Capture, mode: Mode,
	matched_type: Option<&TokenStream>,
) {
	let count = Cell::new(0);
	let ctx = Context::new(&count, mode, matched_type);
	let container = cap.info.as_ref().map_or(CapContainer::None, |info| info.container);

	chunk!(stream,
		let mut #{ident!("__cap__{}", cap.ident)} = None;
		let mut __res = match #do { gen_cap_inline(stream, cap, &ctx) } {
			Ok(_) => #mode::ok(||
				#do { gen_cap_unwrwap(stream, &cap.ident, container) }
			),
			Err(err) => Err(err),
		};
	);
}
/// generate matching logic for root non capturing [`Expr`], save result in `__res`
fn gen_root_expr(mut stream: &mut TokenStream, expr: &Expr, mode: Mode) {
	let count = Cell::new(0);
	let ctx = Context::new(&count, mode, None);
	chunk!(stream, let mut __res = #do { gen_expr_inline(stream, expr, &ctx) };);
}

/// generate `Matcher::expected`
fn gen_matcher_impl_expected(
	stream: &mut TokenStream, expr: &Expr, matched_type: &TokenStream,
) {
	if let Some(expr) = locate_expected(expr) {
		gen_expected(stream, expr, DEFAULT_EXPECTED_FUEL, Some(matched_type));
	} else {
		chunk!(stream, __::Expected::None);
	}
}

/// generate `Matcher` implementation for a matcher
fn gen_matcher_impl(
	mut stream: &mut TokenStream, matcher_ident: &Ident, cap: &Capture,
	matched_type: &TokenStream, args: &[Ident],
	prologue: impl Fn(&mut TokenStream, bool),
) {
	chunk!(stream,
		# #[allow(nonstandard_style, unused_imports, )]
		impl<#for arg in args #{ #arg: __Matcher<#matched_type>, }>
			__Matcher<#matched_type> for #matcher_ident<#for arg in args #{ #arg, }>
		{
			type Capture<'src> = #{&cap.info.as_ref().unwrap().resolved_type};
			fn do_match<'src, __M: __Mode>(
				&self, __value: &'src #{&matched_type}, __off: &mut usize,
			) -> __::MatchResult<Self::Capture<'src>, __M> {
				#do { prologue(stream, true) }
				#do { gen_root_cap(stream, cap, Mode::param(), Some(&matched_type.clone())) }
				__res
			}
			fn expected(&self) -> __::Expected {
				#do { prologue(stream, false) }
				#do { gen_matcher_impl_expected(stream, &cap.expr, matched_type) }
			}
		}
	);
}

/// generate [`Term`]
pub fn gen_term(mut stream: &mut TokenStream, term: &Term, matched_type: &TokenStream) {
	let args = &term.args;
	let args_t = args.iter().map(pascal_case).collect::<Vec<_>>();
	let matcher_ident = ident!("{}__Matcher", span = term.name.span(), term.name);

	chunk!(stream,
		#if args.is_empty() #{
			# #[allow(nonstandard_style)]
			pub const #{&term.name}: #matcher_ident = #matcher_ident;
		} #else #{
			// fn term<Args>(args) -> Matcher<Args> { matcher(args) }
			pub fn #{&term.name}<#for arg in &args_t #{
				#arg: __Matcher<#matched_type>,
			}>(#for arg in args #{ #arg: #{pascal_case(arg)}, })
				-> #matcher_ident<#for arg in &args_t #{ #arg, }>
			{
				#matcher_ident(#for arg in args #{ #arg, })
			}
		}

		# #[doc(hidden)]
		# #[allow(nonstandard_style)]
		#if args.is_empty() #{
			# #[derive(Clone, Copy)]
		}
		# #[derive(Debug)]
		pub struct #matcher_ident
		#if !args.is_empty() #{
			<#for arg in &args_t #{
				#arg: __Matcher<#matched_type>,
			}>
			(#for arg in &args_t #{ #arg, })
		};
		#do { gen_matcher_impl(stream, &matcher_ident, &term.cap, matched_type, &args_t,
			|mut stream, in_matcher| chunk!(stream,
				#if !args.is_empty() #{
					let Self(#for arg in args #{ #arg, }) = self;
				}
				#if term.cap.map.is_some() && in_matcher #{
					// wihtout this the matcher const would overlap with the matched section binding in mapping
					# #[doc(hidden)]
					fn #{&term.name} () {}
				}
			)
		); }
	);
}

/// generate ananymous `Matcher`
pub fn gen_matcher(mut stream: &mut TokenStream, matcher: &Matcher) {
	let Some(matched_type) = matcher.matched_type.as_ref() else { return };
	chunk!(stream,
		# #[derive(Debug, Clone, Copy)]
		struct Matcher;
		#do { gen_matcher_impl(
			stream, &ident!("Matcher"), &matcher.cap, matched_type, &[], |_, _| ()
		); }
		Matcher
	);
}

/// generate imports for concrete `gramex::Mode`s
fn gen_import_modes(stream: &mut TokenStream, capture: bool, error: bool) {
	match (capture, error) {
		(true, false) => chunk!(stream, use ::gramex::modes::Capture as __Capture;),
		(false, true) => chunk!(stream, use ::gramex::modes::Check as __Check;),
		// `Parse` get derived into `Capture` and `Check`
		(true, true) => chunk!(stream, use ::gramex::modes::{ #do{}
			Capture as __Capture, Check as __Check, Parse as __Parse
		};),
		_ => {}
	}
}

/// generate matching logic for matching expression macros
pub fn gen_match_expr(
	mut stream: &mut TokenStream, capture: bool, error: bool, value: &TokenStream,
	expr: &Expr,
) {
	let mode = Mode { is_concrete: true, capture, error };
	gen_import_modes(stream, capture, error);
	chunk!(stream,
		let __value = #value;
		let __value = __value.__as_matchable();
		let __off = &mut 0;
		#match expr {
			Expr::Capture(cap) => gen_root_cap(stream, cap, mode, None),
			_ => gen_root_expr(stream, expr, mode),
		}

		// check excess
		if __res.is_ok() && *__off != <_ as __MatchAble>::len(__value) {
			__res = #mode::err(|| __::MatchError::excess(*__off));
		}

		__res #match (capture, error) {
			(false, false) => #{ .is_ok() },
			(true, false) => #{ .ok() },
			_ => {}
		}
	);
}

/// cursor operation
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CursorOp {
	Eat,
	TryEat,
	Test,
}

/// generate matching logic for cursor macros
pub fn gen_cursor_op(
	stream: &mut TokenStream, op: CursorOp, cursor: &TokenStream, expr: &Expr,
) {
	let (capture, error) = match op {
		CursorOp::Eat => (true, true),
		CursorOp::TryEat => (true, false),
		CursorOp::Test => (false, false),
	};
	gen_import_modes(stream, capture, error);
	let mode = Mode { is_concrete: true, capture, error };

	chunk!(stream,
		use __::{Cursor as _, AsCursor as _};
		let __cur = (#cursor).__as_cursor();
		let __value = <_ as Cursor>::input(__cur);
		let __off = #if op == CursorOp::Eat #{ <_ as Cursor>::off_mut(__cur) }
					#else #{ &mut <_ as Cursor>::off(__cur) };

		#match expr {
			Expr::Capture(cap) => gen_root_cap(stream, cap, mode, None),
			_ => gen_root_expr(stream, expr, mode),
		}

		#match op {
			CursorOp::Eat => #{ __res },
			CursorOp::TryEat => #{ match __res {
				Ok(res) => { *<_ as Cursor>::off_mut(__cur) = *__off; Some(res) },
				Err(err) => None,
			} },
			CursorOp::Test => #{ __res.is_ok() },
		}
	);
}

/// generate matching logic for `match_map` macro
pub fn gen_match_map(
	mut stream: &mut TokenStream, cursor: &Ident, arms: &[Capture], else_: &TokenStream,
) {
	let count = Cell::new(1);
	let mode = Mode { is_concrete: true, capture: true, error: false };
	let ctx = Context::new(&count, mode, None);

	chunk!(stream,
		use ::gramex::modes::Capture as __Capture;
		use __::{ Cursor as _, AsCursor as _ };
		let #cursor = (#cursor).__as_cursor();
		let __value = <_ as Cursor>::input(#cursor);
		let mut __cap__root = None;

		// use enum capture inspired approuch
		let __start = <_ as Cursor>::off(#cursor);
		#for arm in arms #{
			let __off = <_ as Cursor>::off_mut(#cursor);
			*__off = __start;
			if let Ok(_) = #do { gen_cap_inline(stream, arm, &ctx) } {
				// for 'mat_0, look inside `match_map` root fn
				break 'mat_0 __cap__root.unwrap()
			}
		}
		*<_ as Cursor>::off_mut(#cursor) = __start;
		#else_
	);
}

/// pascal -> camel
fn camel_case(ident: &Ident) -> Ident {
	let orig = ident.to_string();
	let mut res = String::with_capacity(orig.len());

	for (ind, char) in orig.chars().enumerate() {
		if char.is_ascii_uppercase() && ind > 0 {
			res.push('_');
		}
		res.push(char.to_ascii_lowercase());
	}

	Ident::new(&res, ident.span())
}

/// generate `derive_enum_matcher` variant matcher `Matcher::do_match`
fn gen_variant_do_match(
	mut stream: &mut TokenStream, enum_: &Ident, name: &Ident,
	inner: Option<&TokenStream>,
) {
	chunk!(stream,
		let Some(token) = <_ as ::gramex::MatchAble>::get_token(value, *off)
		else { return M::err(||
			::gramex::result::MatchError::incomplete(self.expected(), *off)
		); };

		if let #enum_::#name
			#if inner.is_some() #{ (inner) } #else #{ { .. } }
		= token {
			*off += 1;
			Ok(M::wrap_success(
				#if inner.is_some() #{ inner } #else #{ token }
			))
		} else { M::err(||
			::gramex::result::MatchError::mismatch(self.expected(), *off)
		)}
	);
}

/// generate matching logic for `derive_enum_matcher` macro
pub fn gen_enum_matcher(mut stream: &mut TokenStream, matcher: &EnumMatcher) {
	let EnumMatcher { name: enum_, matched_type, vars } = matcher;
	for Variant { name, inner } in vars {
		let camel_name = camel_case(name);
		let matcher_ident = ident!("{}__Matcher", span = name.span(), name);
		chunk!(stream,
			# #[allow(nonstandard_style)]
			pub const #camel_name: #matcher_ident = #matcher_ident;
			# #[doc(hidden)]
			# #[allow(nonstandard_style)]
			pub struct #matcher_ident;
			impl<'slice> ::gramex::Matcher<#matched_type> for #matcher_ident {
				type Capture<'src> = #match inner {
					Some(inner) => #{ &'src #inner },
					None => #{ <#matched_type as ::gramex::MatchAble>::Token<'src> },
				} where #matched_type: 'src;

				fn do_match<'src, M: ::gramex::Mode>(
					&self, value: &'src #matched_type, off: &mut usize,
				) -> ::gramex::result::MatchResult<Self::Capture<'src>, M> {
					#do { gen_variant_do_match(stream, enum_, name, inner.as_ref()) }
				}
			}
		);
	}
}
