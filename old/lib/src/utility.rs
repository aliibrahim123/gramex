use crate::{MatchAble, MatchBy, MatchResult, MatchSignal, MatchStatus, Matcher};

#[allow(unused_imports)]
use crate::{MatchError, matcher};

/// create a [`Matcher`] from a matching value.
///
/// like [`matcher!`] but normal fn working only on a single matching value.
///
/// # example
/// ```
/// assert!(matches("abc", matcher_for("abc")));
/// ```
#[inline]
pub fn matcher_for<'a, T: MatchAble + ?Sized + MatchBy<&'a M>, M: ?Sized>(
	matching: &'a M,
) -> impl Matcher<T> {
	move |v, i, s| v.match_by(matching, i, s)
}

/// matches a [`MatchAble`] and return the matched and remaining sections.
///
/// it matches a [`MatchAble`] using a [`Matcher`] from the start, then split the value into a `(matched, remaining)` sections, failing if the matcher fails.
///
/// # example
/// ```
/// assert_eq!(consume("abcDEF", matcher!(for str, lower*)), Ok(("abc", "DEF")));
/// ```
pub fn consume<'a, T: MatchAble + ?Sized>(
	value: &'a T, matcher: impl Matcher<T>,
) -> MatchResult<(T::Slice<'a>, T::Slice<'a>)> {
	let mut ind = 0;
	let sig = matcher.do_match(value, &mut ind, &MatchStatus::default());
	if sig == MatchSignal::Matched {
		Ok((value.slice(0..ind), value.slice(ind..value.len())))
	} else {
		Err(sig.into_err(ind))
	}
}

/// provide type inference for closures implementing [`Matcher`].
///
/// when passing a closure to [`MatchBy::match_by`], it lose the type inference since of the heavily generic nature of it.
///
/// # example
/// ```
/// assert!(matches!("": str, {|v: &str, i: &mut usize, s: &MatchStatus| MatchSignal::Matched}));
/// assert!(matches!("": str, {by(|v, i, s| MatchSignal::Matched)}));
/// ```
#[inline]
pub fn by<T: MatchAble + ?Sized, F: Fn(&T, &mut usize, &MatchStatus) -> MatchSignal>(
	matcher: F,
) -> F {
	matcher
}

/// test a predicate without advancing the index.
///
/// `test` creates a [`Matcher`] that call a predicate function with the same argumants as [`Matcher`] to [match](MatchSignal::Matched) or [not match](MatchSignal::MisMatched) based on the returned [`bool`].
///
/// this function doesnt fail on incomplete input.
///
/// # example
/// ```
/// assert!(matches!("abc": str, {test(|v, _, _| v.starts_with('a'))} _+));
/// ```
#[inline]
pub fn test<T: MatchAble + ?Sized>(
	predicate: impl Fn(&T, &mut usize, &MatchStatus) -> bool,
) -> impl Matcher<T> {
	move |v, i, s| {
		if predicate(v, i, s) { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}
}

/// run a function at the current index.
///
/// `touch` doesnt advance the index, or does it fail on incomplete input, it just calls a function with the arguments of [`Matcher`], and always match.
///
/// # example
/// ```
/// assert!(matches!("abc": str, {touch(|v, i, _| println!("{}", v[i..]))} _+));
/// ```
#[inline]
pub fn touch<T: MatchAble + ?Sized>(fun: impl Fn(&T, &mut usize, &MatchStatus)) -> impl Matcher<T> {
	move |v, i, s| {
		fun(v, i, s);
		MatchSignal::Matched
	}
}

/// matches a token using a predicate.
///
/// it extracts a token using [`MatchAble::get_n`], then match it by the predicate.
///
/// # example
/// ```
/// assert!(matches!("abc": str, {a(|v| v.chars().all(char::is_lowercase))}[3]));
/// ```
#[inline]
pub fn a<T: MatchAble + ?Sized>(predicate: impl Fn(T::Slice<'_>) -> bool) -> impl Matcher<T> {
	move |v, i, s| {
		let slice = match v.get_n(i, 1, s) {
			Ok(slice) => slice,
			Err(sig) => return sig,
		};
		if predicate(slice) { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}
}
/// matches number of tokens using a predicate.
///
/// it extracts the token using [`MatchAble::get_n`], then match them by the predicate.
///
/// # example
/// ```
/// assert!(matches("abc", an(3, |v| v.chars().all(char::is_lowercase))));
/// ```
#[inline]
pub fn an<T: MatchAble + ?Sized>(
	n: usize, predicate: impl Fn(T::Slice<'_>) -> bool,
) -> impl Matcher<T> {
	move |v, i, s| {
		let slice = match v.get_n(i, n, s) {
			Ok(slice) => slice,
			Err(sig) => return sig,
		};
		if predicate(slice) { MatchSignal::Matched } else { MatchSignal::MisMatched }
	}
}

/// matches a `sep` separated list of `item`.
///
/// `list` is a [parameterized matcher](crate::docs::glossary#parameterized-matcher), it try to match the `item` list until it fails.
///
/// starting / terminated `sep` are not matched.
///
/// # example
/// ```
/// assert!(matches!("a,b,c": str, list<lower, ','>));
/// assert!(matches!("1-2-3-": str, list<dec, '-'> ','?));
/// ```
#[inline]
pub fn list<T: MatchAble + ?Sized>(
	value: &T, item: impl Matcher<T>, sep: impl Matcher<T>, ind: &mut usize, status: &MatchStatus,
) -> MatchSignal {
	let mut is_first = true;
	loop {
		let start_ind = *ind;
		if !is_first && sep.do_match(value, ind, status) != MatchSignal::Matched {
			*ind = start_ind;
			return MatchSignal::Matched;
		}
		if item.do_match(value, ind, status) != MatchSignal::Matched {
			*ind = start_ind;
			return MatchSignal::Matched;
		}
		is_first = false;
	}
}

/// matches the end of the input.
///
/// the inverse of [`_`](crate::docs::gram_ref#any-_).
///
/// # example
/// ```
/// assert!(matches!("abc": str, !~eof {test(|v, i, _| v[*i..] == "abc")} _+));
/// ```
#[inline]
pub fn eof<T: MatchAble + ?Sized>(value: &T, ind: &mut usize, status: &MatchStatus) -> MatchSignal {
	let _ = status;
	if *ind == value.len() { MatchSignal::Matched } else { MatchSignal::InComplete }
}

/// matches nothing.
///
/// # example
/// ```
/// assert!(matches!("abc": str, 'a' noop 'b' noop 'c'));
/// ```
#[inline]
pub fn noop<T: MatchAble + ?Sized>(
	value: &T, ind: &mut usize, status: &MatchStatus,
) -> MatchSignal {
	let _ = (value, ind, status);
	MatchSignal::Matched
}

/// always fail with [`MatchSignal::MisMatched`].
#[inline]
pub fn fail<T: MatchAble + ?Sized>(
	value: &T, ind: &mut usize, status: &MatchStatus,
) -> MatchSignal {
	let _ = (value, ind, status);
	MatchSignal::MisMatched
}

/// always fail with a custom [`MatchSignal::Error`] message.
///
/// append `at {index}` to the message where `index` is the current index.
///
/// # example
/// ```
/// assert!(try_match!("abc": str, 'a' {fail_with("example error")}).unwrap_err().msg == "example error at 1")
/// ```
#[inline]
pub fn fail_with<T: MatchAble + ?Sized>(msg: &str) -> impl Matcher<T> {
	move |_, ind, _| MatchSignal::Error(format!("{msg} at {ind}"))
}
