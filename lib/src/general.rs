use core::marker::PhantomData;

use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	result::{Expected, IntoResult, MatchError, MatchResult},
};

#[allow(nonstandard_style)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct end;
impl<T: MatchAble + ?Sized> Matcher<T> for end {
	type Capture<'src>
		= ()
	where
		T: 'src;
	#[inline]
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<(), M> {
		if *off == matched.len() {
			Ok(M::wrap_success(()))
		} else {
			M::err(|| MatchError::excess(*off))
		}
	}
}

#[inline]
pub fn atomic<T: MatchAble + ?Sized, U: Matcher<T>>(matcher: U) -> Atomic<U> {
	Atomic(matcher)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atomic<U>(U);
impl<T: MatchAble + ?Sized, U: Matcher<T>> Matcher<T> for Atomic<U> {
	type Capture<'src>
		= U::Capture<'src>
	where
		T: 'src;
	#[inline]
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<U::Capture<'src>, M> {
		let start = *off;
		let res = self.0.do_match::<M>(matched, off);
		if res.is_err() {
			*off = start;
		}
		res
	}
	fn expected(&self) -> Expected {
		self.0.expected()
	}
}

#[inline]
pub fn a<T: MatchAble + ?Sized, F>(pred: F) -> A<T, F>
where
	F: for<'src> Fn(T::Token<'src>) -> bool,
	for<'src> T::Token<'src>: Clone,
{
	A { pred, __marker: PhantomData }
}
#[derive(Debug, Clone, Copy)]
pub struct A<T: ?Sized, F> {
	pred: F,
	__marker: PhantomData<fn(&T)>,
}
impl<T: MatchAble + ?Sized, F> Matcher<T> for A<T, F>
where
	F: for<'src> Fn(T::Token<'src>) -> bool,
	for<'src> T::Token<'src>: Clone,
{
	type Capture<'src>
		= T::Token<'src>
	where
		T: 'src;
	#[inline]
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<T::Token<'src>, M> {
		let start = *off;
		matched.skip_n::<M::WithoutCapture>(off, 1)?;
		let token = matched.get_token(start).unwrap();
		if (self.pred)(token.clone()) {
			Ok(M::wrap_success(token))
		} else {
			*off = start;
			M::err(|| MatchError::mismatch(Expected::None, start))
		}
	}
}

#[inline]
pub fn an<T: MatchAble + ?Sized, F>(n: usize, pred: F) -> An<T, F>
where
	F: for<'src> Fn(T::Slice<'src>) -> bool,
	for<'src> T::Slice<'src>: Clone,
{
	An { pred, n, __marker: PhantomData }
}
#[derive(Debug, Clone, Copy)]
pub struct An<T: ?Sized, F> {
	pred: F,
	n: usize,
	__marker: PhantomData<fn(&T)>,
}
impl<T: MatchAble + ?Sized, F> Matcher<T> for An<T, F>
where
	F: for<'src> Fn(T::Slice<'src>) -> bool,
	for<'src> T::Slice<'src>: Clone,
{
	type Capture<'src>
		= T::Slice<'src>
	where
		T: 'src;
	#[inline]
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<T::Slice<'src>, M> {
		let An { pred, n, .. } = self;
		let start = *off;
		matched.skip_n::<M::WithoutCapture>(off, *n)?;
		let slice = matched.slice(start..*off).unwrap();
		if pred(slice.clone()) {
			Ok(M::wrap_success(slice))
		} else {
			*off = start;
			M::err(|| MatchError::mismatch(Expected::None, start))
		}
	}
}

pub fn expected(expected: impl Into<Expected>) -> FailExpected {
	FailExpected(expected.into())
}
#[derive(Debug, Clone, PartialEq)]
pub struct FailExpected(Expected);
impl<T: MatchAble + ?Sized> Matcher<T> for FailExpected {
	type Capture<'src>
		= ()
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<(), M> {
		if *off == matched.len() {
			M::err(|| MatchError::incomplete(self.0.clone(), *off))
		} else {
			M::err(|| MatchError::mismatch(self.0.clone(), *off))
		}
	}
	fn expected(&self) -> Expected {
		self.0.clone()
	}
}

pub fn fail_with(msg: impl Into<LeanString>) -> FailWith {
	FailWith(msg.into())
}
#[derive(Debug, Clone, PartialEq)]
pub struct FailWith(LeanString);
impl<T: MatchAble + ?Sized> Matcher<T> for FailWith {
	type Capture<'src>
		= ()
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, _matched: &'src T, off: &mut usize,
	) -> MatchResult<(), M> {
		M::err(|| MatchError::other(self.0.clone(), *off))
	}
}

#[inline]
pub fn list<T: MatchAble + ?Sized, Item: Matcher<T>, Sep: Matcher<T>>(
	item: Item, sep: Sep,
) -> List<Item, Sep> {
	List(item, sep)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct List<Item, Sep>(Item, Sep);
impl<T: MatchAble + ?Sized, Item: Matcher<T>, Sep: Matcher<T>> Matcher<T>
	for List<Item, Sep>
{
	type Capture<'src>
		= Vec<Item::Capture<'src>>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		let Self(item, sep) = self;
		let mut items = Vec::new();
		loop {
			let item = item.do_match::<M>(matched, off)?;
			if M::DO_CAPTURE {
				items.push(M::unwrap_success(item));
			}
			if !atomic(sep).test(matched, off) {
				break;
			}
		}
		M::ok(|| items)
	}
	fn expected(&self) -> Expected {
		self.0.expected()
	}
}

#[inline]
pub fn delim_list<T, Start, Item, Sep, End>(
	start: Start, item: Item, sep: Sep, _end: End,
) -> DelimList<Start, Item, Sep, End>
where
	T: MatchAble + ?Sized,
	Start: Matcher<T>,
	Item: Matcher<T>,
	Sep: Matcher<T>,
	End: Matcher<T>,
{
	DelimList(start, item, sep, _end)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelimList<Start, Item, Sep, End>(Start, Item, Sep, End);
impl<
	T: MatchAble + ?Sized,
	Start: Matcher<T>,
	Item: Matcher<T>,
	Sep: Matcher<T>,
	End: Matcher<T>,
> Matcher<T> for DelimList<Start, Item, Sep, End>
{
	type Capture<'src>
		= Vec<Item::Capture<'src>>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		let Self(start, item, sep, _end) = self;
		let mut items = Vec::new();
		start.do_match::<M::WithoutCapture>(matched, off)?;
		loop {
			if atomic(_end).test(matched, off) {
				break;
			}
			let item = item.do_match::<M>(matched, off)?;
			if M::DO_CAPTURE {
				items.push(M::unwrap_success(item));
			}
			if !atomic(sep).test(matched, off) {
				_end.do_match::<M::WithoutCapture>(matched, off)?;
				break;
			}
		}
		M::ok(|| items)
	}
	fn expected(&self) -> Expected {
		self.0.expected()
	}
}

#[doc(hidden)]
pub trait LifedMatchFn<'src, T: MatchAble + ?Sized + 'src> {
	type Capture: 'src;
	type Res: IntoResult<Output = Self::Capture>;
	fn call(&self, matched: &'src T, off: &mut usize) -> Self::Res;
}

impl<'src, T: MatchAble + ?Sized + 'src, R, F> LifedMatchFn<'src, T> for F
where
	F: Fn(&'src T, &mut usize) -> R,
	R: IntoResult,
	<R as IntoResult>::Output: 'src,
{
	type Capture = <R as IntoResult>::Output;
	type Res = R;

	fn call(&self, matched: &'src T, off: &mut usize) -> R {
		self(matched, off)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MatchFn<T: MatchAble + ?Sized, F> {
	fun: F,
	_marker: PhantomData<fn(&T)>,
}
impl<T: MatchAble + ?Sized, F> MatchFn<T, F>
where
	F: for<'src> LifedMatchFn<'src, T>,
{
	pub fn new(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<T: MatchAble + ?Sized, F, G> MatchFn<T, F>
where
	F: Fn(&T, &mut usize) -> G,
	G: IntoResult,
{
	#[doc(hidden)]
	pub fn new_with_infer(fun: F) -> Self {
		Self { fun, _marker: PhantomData }
	}
}
impl<T: MatchAble + ?Sized, F> Matcher<T> for MatchFn<T, F>
where
	F: for<'src> LifedMatchFn<'src, T>,
{
	type Capture<'src>
		= <F as LifedMatchFn<'src, T>>::Capture
	where
		T: 'src;

	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M> {
		LifedMatchFn::call(&self.fun, matched, off).into_result::<M>(*off)
	}
}
