//! general [`Matcher`]s that work on all [`MatchAble`]s.

use core::marker::PhantomData;

use lean_string::LeanString;

use crate::{
	MatchAble, Matcher, Mode,
	general::matchers::{
		A, An, Atomic, DelimList, End, FailExpected, FailWith, List, Pos,
	},
	result::{Expected, IntoResult, MatchResult},
};

/// matches the end of the input.
///
/// `end` is a universal [`Matcher`] that matches with `()` when the `off == value.len()`, otherwise it fails with [excess](crate::result::MatchErrorKind::Excess) [`MatchError`](crate::result::MatchError).
///
/// # example
/// ```
/// assert_eq!(parse!("abc", "abc" end), Ok(()));
/// assert_eq!(parse!("abcd", "abc" end), Err(MatchError::excess(3)));
/// ```
#[allow(nonstandard_style)]
pub const end: End = End;

/// capture the current offset.
///
/// `pos` is a universal [`Matcher`] that always matches with the current offset.
///
/// # example
/// ```
/// let (start, end) = try_match!("abc", 'a' start:pos "bc" end:pos).unwrap();
/// assert_eq!((start, end), (1, 3));
/// ```
#[allow(nonstandard_style)]
pub const pos: Pos = Pos;

/// turn a [`Matcher`] into an atomic [`Matcher`].
///
/// `atomic` takes a [`Matcher`] and returns a new [`Matcher`] that redirect matching to the input, and rewind to start offset on failure.
///
/// # example
/// ```
/// let (mut off1, mut off2) = (0, 0);
/// assert_eq!("abc".parse("abc", &mut off1), atomic("abc").parse("abc", &mut off2));
/// assert_eq!(off1, off2);
///
/// (off1, off2) = (0, 0);
/// assert_eq!("abc".parse("abd", &mut off1), atomic("abc").parse("abd", &mut off2));
/// assert_eq!((off1, off2), (2, 0));
/// ```
#[inline]
pub fn atomic<T: MatchAble + ?Sized, U: Matcher<T>>(matcher: U) -> Atomic<T, U> {
	Atomic(matcher, PhantomData)
}

/// matches a [token](MatchAble::Token) with a predicate.
///
/// `a` produce a [`Matcher`] that capture the current [token](MatchAble::Token) and test it by `pred`. if `pred` return `true` it matches with the token, else it fail.
///
/// # example
/// ```
/// let is_alpha = a(|c| c.is_alpha);
/// assert_eq!(try_match("a", is_alpha), Ok('a'));
/// assert_eq!(try_match("1", is_alpha), None);
/// assert_eq!(try_match("", is_alpha), None);
/// ```
#[inline]
pub fn a<T: MatchAble + ?Sized, F>(pred: F) -> A<T, F>
where
	F: for<'src> Fn(T::Token<'src>) -> bool,
	for<'src> T::Token<'src>: Clone,
{
	A { pred, __marker: PhantomData }
}

/// matches a [slice](MatchAble::Slice) of `n` [tokens](MatchAble::Token) with a predicate.
///
/// `an` produce a [`Matcher`] that capture a [slice](MatchAble::Slice) of `n` [tokens](MatchAble::Token) and test it by `pred`. if `pred` return `true` it matches with the slice, else it fail.
///
/// # example
/// ```
/// let is_4_lower = an(4, |c| c.chars().all(char::is_lowercase));
/// assert_eq!(try_match("abcd", is_4_lower), Ok("abcd"));
/// assert_eq!(try_match("Abcd", is_4_lower), None);
/// assert_eq!(try_match("abc", is_4_lower), None);
/// ```
#[inline]
pub fn an<T: MatchAble + ?Sized, F>(n: usize, pred: F) -> An<T, F>
where
	F: for<'src> Fn(T::Slice<'src>) -> bool,
	for<'src> T::Slice<'src>: Clone,
{
	An { pred, n, __marker: PhantomData }
}

/// always fail with [`Expected`].
///
/// `expected` produce a [`Matcher`] that always fail with [mismatch](crate::result::MatchErrorKind::MisMatch) and [incomplete](crate::result::MatchErrorKind::InComplete) [`MatchError`](crate::result::MatchError) with `expected` depending on the offset.
///
/// # example
/// ```
/// assert_eq!(
///     check("abc", expected("thing")),
///     Err(MatchError::mismatch("thing".into(), 0)),
/// );
/// assert_eq!(
///     check("", expected("thing")),
///     Err(MatchError::incomplete("thing".into(), 0)),
/// );
/// ```
pub fn expected(expected: impl Into<Expected>) -> FailExpected {
	FailExpected(expected.into())
}

/// always fail with a custom message.
///
/// `fail_with` produce a [`Matcher`] that always fail with a [Other](crate::result::MatchErrorKind::Other) [`MatchError`](crate::result::MatchError) of custom `msg`.
///
/// # example
/// ```
/// assert_eq!(
///     check("abc", fail_with("custom error")),
///     Err(MatchError::other("custom error", 0)),
/// );
/// ```
pub fn fail_with(msg: impl Into<LeanString>) -> FailWith {
	FailWith(msg.into())
}

/// matches a separated list of `item`s.
///
/// `list` take an `item` and `sep` [`Matcher`]s and produce a [`Matcher`] that matches a non-empty list of `item`s separated by `sep`.
///
/// no trailing comma is allowed, and the capture is `Vec<Item::Capture>`.
///
/// # example
/// ```
/// let alpha_list = list(alpha, ',');
/// assert_eq!(try_match("a,b,c", alpha_list), Some(vec!["a", "b", "c"]));
/// assert_eq!(try_match("a", alpha_list), Some(vec!["a"]));
/// assert_eq!(try_match("a,1,c", alpha_list), None);
/// assert_eq!(try_match("a,", alpha_list), None);
/// assert_eq!(try_match("", alpha_list), None);
/// assert_eq!(
///     try_match!(for str, "a,b,c;", list:list<alpha, ','> ';'),
///     Some((vec!["a", "b", "c"],)),
/// );
/// ```
#[inline]
pub fn list<T: MatchAble + ?Sized, Item: Matcher<T>, Sep: Matcher<T>>(
	item: Item, sep: Sep,
) -> List<T, Item, Sep> {
	List { item, sep, __marker: PhantomData }
}

/// matches a delimited and separated list of `item`s.
///
/// `delim_list` take a `start`, `item`, `sep` and `end` [`Matcher`]s and produce a [`Matcher`] that matches a maybe empty list of `item`s separated by `sep` and delimited by `start` and `end`.
///
/// no trailing comma is allowed, and the capture is `Vec<Item::Capture>`.
///
/// # example
/// ```
/// let alpha_list = delim_list('(', alpha, ',', ')');
/// assert_eq!(try_match("(a,b,c)", alpha_list), Some(vec!["a", "b", "c"]));
/// assert_eq!(try_match("(a)", alpha_list), Some(vec!["a"]));
/// assert_eq!(try_match("()", alpha_list), Some(vec![]));
/// assert_eq!(try_match("(a,1,c)", alpha_list), None);
/// assert_eq!(try_match("(a,)", alpha_list), None);
/// assert_eq!(try_match("(a,b,c", alpha_list), None);
/// assert_eq!(try_match("a,b,c)", alpha_list), None);
/// assert_eq!(
///     try_match!(for str, "(a,b,c);", list:delim_list<'(', alpha, ',', ')'> ';'),
///     Some((vec!["a", "b", "c"],)),
/// );
/// ```
#[inline]
pub fn delim_list<T, Start, Item, Sep, End>(
	start: Start, item: Item, sep: Sep, end_: End,
) -> DelimList<T, Start, Item, Sep, End>
where
	T: MatchAble + ?Sized,
	Start: Matcher<T>,
	Item: Matcher<T>,
	Sep: Matcher<T>,
	End: Matcher<T>,
{
	DelimList { start, item, sep, end_, __marker: PhantomData }
}

/// [`MatchFn`] function of spicific lifetime instance.
#[doc(hidden)]
pub trait LifedMatchFn<'src, T: MatchAble + ?Sized + 'src> {
	type Capture: 'src;
	type Res: IntoResult<Success = Self::Capture>;
	fn call(&self, matched: &'src T, off: &mut usize) -> Self::Res;
}

impl<'src, T: MatchAble + ?Sized + 'src, R, F> LifedMatchFn<'src, T> for F
where
	F: Fn(&'src T, &mut usize) -> R,
	R: IntoResult,
	<R as IntoResult>::Success: 'src,
{
	type Capture = <R as IntoResult>::Success;
	type Res = R;

	fn call(&self, matched: &'src T, off: &mut usize) -> R {
		self(matched, off)
	}
}

/// matches by a function.
///
/// `MatchFn` matches by a function of type `Fn(&MatchAble, &mut usize) -> IntoResult` where the return type is anything implementing [`IntoResult`], like [`bool`], [`Option<T>`] and [`Result<T, MatchError>`].
///
/// due to limitations in rustc, closures must specify their arguments types, and can only capture owned values.
///
/// wrapped functions dont get [`Mode`] generifications treatment, for that it is advised to use regular [`Matcher`]s for general cases.
///
/// # example
/// ```
/// let lt_100 = MatchFn::new(|matched: &str, off: &mut usize| {
///    let len = matched[*off..].find(|char; char| !char.is_digit(10))
///         .unwrap_or(matched.len() - *off);
///     matched[*off..*off + len].parse::<u8>().ok()
///         .filter(|nb| *nb < 100)
///         .inspect(|_| *off += len)
/// });
///
/// assert_eq!(try_match("12", lt_100), Some(12));
/// assert_eq!(try_match("123", lt_100), None);
/// assert_eq!(try_match("abc", lt_100), None);
/// ```
#[derive(Debug)]
pub struct MatchFn<T: MatchAble + ?Sized, F> {
	fun: F,
	_marker: PhantomData<fn(&T)>,
}
impl<T: MatchAble + ?Sized, F: Clone> Clone for MatchFn<T, F> {
	fn clone(&self) -> Self {
		Self { fun: self.fun.clone(), _marker: PhantomData }
	}
}
impl<T: MatchAble + ?Sized, F: Copy> Copy for MatchFn<T, F> {}
impl<T: MatchAble + ?Sized, F> MatchFn<T, F>
where
	F: for<'src> LifedMatchFn<'src, T>,
{
	/// create a new [`MatchFn`] wrapping a `fun`ction.
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
	pub fn new_infer(fun: F) -> Self {
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

/// [general](crate::general) items [`Matcher`]s
pub mod matchers {
	use core::marker::PhantomData;

	use lean_string::LeanString;

	#[allow(clippy::wildcard_imports)]
	use crate::{
		MatchAble, Matcher, Mode,
		general::*,
		result::{Expected, MatchError, MatchResult},
	};

	/// [`end`] [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct End;
	impl<T: MatchAble + ?Sized> Matcher<T> for End {
		type Capture<'src>
			= ()
		where
			T: 'src;
		#[inline]
		fn do_match<M: Mode>(&self, matched: &T, off: &mut usize) -> MatchResult<(), M> {
			if *off == matched.len() {
				Ok(M::wrap_success(()))
			} else {
				M::err(|| MatchError::excess(*off))
			}
		}
	}

	/// [`pos`] [`Matcher`]
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct Pos;
	impl<T: MatchAble + ?Sized> Matcher<T> for Pos {
		type Capture<'src>
			= usize
		where
			T: 'src;
		#[inline]
		fn do_match<M: Mode>(&self, _: &T, off: &mut usize) -> MatchResult<usize, M> {
			Ok(M::wrap_success(*off))
		}
	}

	/// [`atomic`] [`Matcher`]
	#[derive(Debug)]
	pub struct Atomic<T: ?Sized, U>(pub(crate) U, pub(crate) PhantomData<fn(T)>);
	impl<T: ?Sized, U: Clone> Clone for Atomic<T, U> {
		fn clone(&self) -> Self {
			Self(self.0.clone(), self.1)
		}
	}
	impl<T: ?Sized, U: Copy> Copy for Atomic<T, U> {}
	impl<T: MatchAble + ?Sized, U: Matcher<T>> Matcher<T> for Atomic<T, U> {
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

	/// [`a`] [`Matcher`]
	#[derive(Debug)]
	pub struct A<T: ?Sized, F> {
		pub(crate) pred: F,
		pub(crate) __marker: PhantomData<fn(&T)>,
	}
	impl<T: ?Sized, F: Clone> Clone for A<T, F> {
		fn clone(&self) -> Self {
			Self { pred: self.pred.clone(), __marker: PhantomData }
		}
	}
	impl<T: ?Sized, F: Copy> Copy for A<T, F> {}
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

	/// [`an`] [`Matcher`]
	#[derive(Debug)]
	pub struct An<T: ?Sized, F> {
		pub(crate) pred: F,
		pub(crate) n: usize,
		pub(crate) __marker: PhantomData<fn(&T)>,
	}
	impl<T: ?Sized, F: Clone> Clone for An<T, F> {
		fn clone(&self) -> Self {
			Self { pred: self.pred.clone(), n: self.n, __marker: PhantomData }
		}
	}
	impl<T: ?Sized, F: Copy> Copy for An<T, F> {}
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

	/// [`expected`] [`Matcher`]
	#[derive(Debug, Clone, PartialEq)]
	pub struct FailExpected(pub(crate) Expected);
	impl<T: MatchAble + ?Sized> Matcher<T> for FailExpected {
		type Capture<'src>
			= ()
		where
			T: 'src;
		fn do_match<M: Mode>(&self, matched: &T, off: &mut usize) -> MatchResult<(), M> {
			M::err(|| MatchError::expected(self.0.clone(), *off == matched.len(), *off))
		}
		fn expected(&self) -> Expected {
			self.0.clone()
		}
	}

	/// [`fail_with`] [`Matcher`]
	#[derive(Debug, Clone, PartialEq)]
	pub struct FailWith(pub(crate) LeanString);
	impl<T: MatchAble + ?Sized> Matcher<T> for FailWith {
		type Capture<'src>
			= ()
		where
			T: 'src;
		fn do_match<M: Mode>(&self, _matched: &T, off: &mut usize) -> MatchResult<(), M> {
			M::err(|| MatchError::other(self.0.clone(), *off))
		}
	}

	/// [`list`] [`Matcher`]
	#[derive(Debug, PartialEq)]
	pub struct List<T: ?Sized, Item, Sep> {
		pub(crate) item: Item,
		pub(crate) sep: Sep,
		pub(crate) __marker: PhantomData<fn(&T)>,
	}
	impl<T: ?Sized, Item: Clone, Sep: Clone> Clone for List<T, Item, Sep> {
		fn clone(&self) -> Self {
			Self {
				item: self.item.clone(),
				sep: self.sep.clone(),
				__marker: self.__marker,
			}
		}
	}
	impl<T: ?Sized, Item: Copy, Sep: Copy> Copy for List<T, Item, Sep> {}
	impl<T: MatchAble + ?Sized, Item: Matcher<T>, Sep: Matcher<T>> Matcher<T>
		for List<T, Item, Sep>
	{
		type Capture<'src>
			= Vec<Item::Capture<'src>>
		where
			T: 'src;
		fn do_match<'src, M: Mode>(
			&self, matched: &'src T, off: &mut usize,
		) -> MatchResult<Self::Capture<'src>, M> {
			let Self { item, sep, .. } = self;
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
			self.item.expected()
		}
	}

	/// [`delim_list`] [`Matcher`]
	#[derive(Debug)]
	pub struct DelimList<T: ?Sized, Start, Item, Sep, End> {
		pub(crate) start: Start,
		pub(crate) item: Item,
		pub(crate) sep: Sep,
		pub(crate) end_: End,
		pub(crate) __marker: PhantomData<fn(&T)>,
	}
	impl<T: ?Sized, Start: Clone, Item: Clone, Sep: Clone, End: Clone> Clone
		for DelimList<T, Start, Item, Sep, End>
	{
		fn clone(&self) -> Self {
			Self {
				start: self.start.clone(),
				item: self.item.clone(),
				sep: self.sep.clone(),
				end_: self.end_.clone(),
				__marker: self.__marker,
			}
		}
	}
	impl<T: ?Sized, Start: Copy, Item: Copy, Sep: Copy, End: Copy> Copy
		for DelimList<T, Start, Item, Sep, End>
	{
	}
	impl<
		T: MatchAble + ?Sized,
		Start: Matcher<T>,
		Item: Matcher<T>,
		Sep: Matcher<T>,
		End: Matcher<T>,
	> Matcher<T> for DelimList<T, Start, Item, Sep, End>
	{
		type Capture<'src>
			= Vec<Item::Capture<'src>>
		where
			T: 'src;
		fn do_match<'src, M: Mode>(
			&self, matched: &'src T, off: &mut usize,
		) -> MatchResult<Self::Capture<'src>, M> {
			let Self { start, item, sep, end_, .. } = self;
			let mut items = Vec::new();
			start.do_match::<M::WithoutCapture>(matched, off)?;
			if atomic(end_).test(matched, off) {
				return M::ok(|| items);
			}
			loop {
				let item = item.do_match::<M>(matched, off)?;
				if M::DO_CAPTURE {
					items.push(M::unwrap_success(item));
				}
				if !atomic(sep).test(matched, off) {
					end_.do_match::<M::WithoutCapture>(matched, off)?;
					break;
				}
			}
			M::ok(|| items)
		}
		fn expected(&self) -> Expected {
			self.start.expected()
		}
	}
}
