use core::ops::Range;

use alloc_crate::{rc::Rc, sync::Arc};

use crate::result::{Expected, MatchError, MatchResult};

/// a type supporting matching.
///
/// the `MatchAble` trait enables a type to be matchable by gramex, by implementing a set of items and conventions.
///
/// # requirements
/// to be `MatchAble`, the impleminting type must be:
/// - veiwable as token list
/// - indexable by `usize`
/// - supporting random access
/// - support slicing.
///
/// this requirements are essintial for ensuring flixible matching and zero copy parsing are universal property of matching.
///
/// `MatchAble`s can have variable length tokens, not neccessary be unitary sized.
///
/// # implementation example
/// ```
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// enum Token {
///     Nb(i64),
///     Add,
///     Sub,
///     Eq,
///     // ...
/// }
/// // most `MatchAble`s are actually slices
/// #[derive(PartialEq)]
/// struct Tokens<'src>(&'src [Token]);
/// impl MatchAble for Tokens<'_> {
///     type Token<'src> = Token where Self: 'src;
///     type Slice<'src> = Tokens<'src> where Self: 'src;
///     fn len(&self) -> usize {
///         self.0.len()
///     }
///     fn slice(&self, range: std::ops::Range<usize>) -> Option<Tokens<'_>> {
///         self.0.get(range).map(Tokens)
///     }
///     fn get_token(&self, off: usize) -> Option<Token> {
///         self.0.get(off).copied()
///     }
/// }
/// ```
///
/// for slice based types, `MatchAble` can be automatically generated with extra [`Matcher`]s by [`derive_slice_matchable!`](crate::derive::derive_slice_matchable).
/// ```
/// gramex::derive::derive_slice_matchable!(Tokens(&[Token]), true);
/// ```
///
/// # builtin `MatchAble` types
/// gramex implement `MatchAble` for multiple types based on feature flags:
/// - [`str`] though [`str` module](crate::str) and `str` feature flag.
/// - [`[u8]`] though [`bytes` module](crate::bytes) and `bytes` feature flag.
/// - bitfields though [`bits` module](crate::bits) and `bits` feature flag.
pub trait MatchAble {
	/// a single token of the `Matchable`` created by [`get_token`](Self::get_token).
	///
	/// `Token` can be [`Copy`], referece or anything.
	///
	/// it must be linked to the lifetime of the `Matchable` through `'src` lifetime.
	type Token<'src>
	where
		Self: 'src;

	/// a slice of the `Matchable` tokens, created by [`slice`](Self::slice).
	///
	/// `Slice` can be of anytype, but usually primitive [slices](primitive@slice).
	///
	/// it must be linked to the lifetime of the `Matchable` through `'src` lifetime.
	///
	/// implimiting `MatchAble` for it provide the `MatchAble` subslice matching ability, and [`and expression`](crate::gram_ref#and-expression) support.
	type Slice<'src>
	where
		Self: 'src;

	/// the length of the `Matchable``
	///
	/// the length must be stable during matching and signal the end of input when the offset hit it.
	///
	/// but it doesnt need to equal the token count.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// assert_eq!(tokens.len(), 3);
	/// ```
	fn len(&self) -> usize;

	/// slice the `Matchable` by a [`Range`].
	///
	/// `slice` try slice the `MatchAble` into a [`Slice`](Self::Slice), returning `None` if out of bound or unaligned to token boundries.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// assert_eq!(tokens.slice(1..3), Some(Tokens(&[Token::Add, Token::Nb(2)])));
	/// assert_eq!(tokens.slice(1..4), None);
	/// ```
	fn slice(&self, range: Range<usize>) -> Option<Self::Slice<'_>>;

	/// get the [`Token`](Self::Token) at the given offset.
	///
	/// `get_token` returns `None` if out of bound or unaligned to token boundries.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// assert_eq!(tokens.get_token(1), Some(Token::Add));
	/// assert_eq!(tokens.get_token(3), None);
	/// ```
	fn get_token(&self, off: usize) -> Option<Self::Token<'_>>;

	/// move a given offset forward by `n` tokens.
	///
	/// `skip_n` is generic over [`Mode`], and return a [`MatchError`] if cant skip all `n` tokens.
	///
	/// the default implementation treats the `MatchAble` as having unitary sized tokens, and returns [incomplete](crate::result::MatchErrorKind::InComplete) [`MatchError`] if there are not enough tokens.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	/// assert_eq!(tokens.skip_n::<Test>(&mut off, 2), Ok(()));
	/// assert_eq!(off, 2);
	/// assert!(tokens.skip_n::<Test>(&mut off, 3).is_err());
	/// ```
	fn skip_n<M: Mode>(&self, off: &mut usize, n: usize) -> MatchResult<(), M> {
		if *off + n > self.len() {
			M::err(|| MatchError::incomplete(Expected::SomeThing, *off))
		} else {
			*off += n;
			Ok(M::wrap_success(()))
		}
	}
}

/// matching mode type state.
///
/// the `Mode` trait utilize GATs (generic associated types) and type state pattern to generic the matching logic over matching features.
///
/// it is generally used in [`Matcher`]s to have one super implementation that get monomorphizied with only the required features and no extra bloat, supporting everything from quick tester to advance parsers from the same primitive units.
///
/// # how it work
/// `Mode` allows the generification of features though orthogonal set of associated items for each feature that acts as selectors and guards.
///
/// there are 2 features [capture](#capture-feature) and [error](#error-feature).
///
/// each feature has:
/// - its own type `T` and [`MatchResult`] variant.
/// - `type Result`: the result type of the feature, `T` when enabled and `()` when disabled.
/// - `const DO_FEATURE: bool`: refelct the feature state.
/// - `type With/WithoutFeature`: the `Mode` with the feature enabled/disabled.
/// - `fn result(res: FnOnce() -> T) -> MatchResult`: construct a [`MatchResult`] from the result of `res`, discarding `res` if the feature is disabled.
/// - `fn wrap_result(res: T) -> Result`: wrap `T` in `Result`, dicarding `T` if the feature is disabled.
/// - `fn unwrap_result(res: Result) -> T`: unwrap `T` from `Result` only if the feature is enabled.
///
/// # `capture` feature.
/// the `capture` feature controls the production of [`Capture`s](Matcher::Capture) during matching.
///
/// its items are:
/// - any `T` as own type, and [`Ok`] [`MatchResult`] variant.
/// - [`Success<T>`](Self::Success) as `type Result`.
/// - [`DO_CAPTURE`](Self::DO_CAPTURE) as `const DO_FEATURE`.
/// - [`WithCapture`](Self::WithCapture)/[`WithoutCapture`](Self::WithoutCapture] as `type WithFeature/WithoutFeature`.
/// - [`ok`](Self::ok) as `fn result`.
/// - [`wrap_success`](Self::wrap_success) as `fn wrap_result`.
/// - [`unwrap_success`](Self::unwrap_success) as `fn unwrap_result`.
///
/// ### example
/// ```
/// fn do_capture<M: Mode>(tokens: &Tokens, matcher: Token) -> Option<M::Success<Token>> {
///     let cap = None;
///     let res = matcher.do_match::<M>(tokens, &mut 0).ok()?;
///     if M::DO_CAPTURE {
///         cap = Some(M::unwrap_success(res));
///     }
///     // ...
///     cap.map(|cap| M::wrap_success(cap))
/// }
///
/// let tokens = Tokens(&[Token::Nb(1)]);
/// assert_eq!(do_capture::<Capture>(&tokens, Token::Nb(1)), Some(Token::Nb(1)));
/// assert_eq!(do_capture::<Test>(&tokens, Token::Nb(1)), Some(()));
/// ```
///
/// # `error` feature
/// the `error` feature controls the production of [`MatchError`] during matching.
///
/// its items are:
/// - [`MatchError`] as `T` and [`Err`] [`MatchResult`] variant.
/// - [`Error`](Self::Error) as `type Result`.
/// - [`DO_ERROR`](Self::DO_ERROR) as `const DO_FEATURE`.
/// - [`WithError`](Self::WithError)/[`WithoutError`](Self::WithoutError] as `type WithFeature/WithoutFeature`.
/// - [`err`](Self::err) as `fn result`.
/// - [`wrap_error`](Self::wrap_error) as `fn wrap_result`.
/// - [`unwrap_error`](Self::unwrap_error) as `fn unwrap_result`.
///
/// ### example
/// ```
/// fn do_error<M: Mode>(tokens: &Tokens, matcher: Token) -> MatchResult<Token, M> {
///     println!("do_error: {}", M::DO_ERROR);
///     match matcher.do_match::<M::WithoutError>(tokens, &mut 0) {
///         Ok(res) => Ok(res),
///         Err(()) => M::err(|| MatchError::mismatch(Expected::SomeThing, 0)),
///     }
/// }
///
/// let tokens = Tokens(&[Token::Nb(1)]);
/// assert_eq!(
///     do_error::<Parse>(&tokens, Token::Add),
///     Err(MatchError::mismatch(Expected::SomeThing, 0))
/// );
/// assert_eq!(do_error::<Test>(&tokens, Token::Add), Err(()));
/// ```
///
/// # concrete `Mode`s
/// gramex define every possible permutation of `Mode` as concrete types, they are:
///
/// | `Mode`      | `capture` | `error` | simplified result        | `use cases`       |
/// | ----------- | --------- | ------- | ------------------------ | ----------------- |
/// | [`Test`]    | `false`   | `false` | `bool`                   | peak based tests  |
/// | [`Capture`] | `true`    | `false` | `Option<T>`              | quick extractions |
/// | [`Check`]   | `false`   | `true`  | `Result<(), MatchError>` | error emit mode   |
/// | [`Parse`]   | `true`    | `true`  | `Result<T, MatchError>`  | advance parsing   |
pub trait Mode {
	/// the result type of the [`capture`](#capture-feature) feature.
	///
	/// it is `T` if `capture` is enabled, otherwise it is `()`.
	type Success<T>;

	/// the result type of the [`error`](#error-feature) feature.
	///
	/// it is [`MatchError]` if `error` is enabled, otherwise it is `()`.
	type Error;

	/// the state of the [`capture`](#capture-feature) feature.
	///
	/// # example
	/// ```
	/// if M::DO_CAPTURE {
	///     println!("captures are enabled");
	/// }
	/// ```
	const DO_CAPTURE: bool;

	/// the state of the [`error`](#error-feature) feature.
	///
	/// # example
	/// ```
	/// if M::DO_ERROR {
	///     println!("errors are enabled");
	/// }
	/// ```
	const DO_ERROR: bool;

	/// this `Mode` variant with the [`capture`](#capture-feature) feature enabled.
	///
	/// # example
	/// ```
	/// assert_eq!(Check::WithCapture, Parse);
	/// assert_eq!(Parse::WithCapture, Parse);
	/// ```
	type WithCapture: Mode<Error = Self::Error>;

	/// this `Mode` variant with the [`error`](#error-feature) feature enabled.
	///
	/// # example
	/// ```
	/// assert_eq!(Test::WithError, Check);
	/// assert_eq!(Check::WithError, Check);
	/// ```
	type WithError: Mode<Error = MatchError>;

	/// this `Mode` variant with the [`capture`](#capture-feature) feature disabled.
	///
	/// # example
	/// ```
	/// assert_eq!(Parse::WithoutCapture, Check);
	/// assert_eq!(Check::WithoutCapture, Check);
	/// ```
	type WithoutCapture: Mode<Error = Self::Error>;

	/// this `Mode` variant with the [`error`](#error-feature) feature disabled.
	///
	/// # example
	/// ```
	/// assert_eq!(Check::WithoutError, Test);
	/// assert_eq!(Capture::WithoutError, Capture);
	/// ```
	type WithoutError: Mode<Error = ()>;

	/// transform the result of `cap` into [`Ok`] based on [`capture`](#capture-feature) feature.
	///
	/// it returns `Ok(cap()) if `capture` is enabled, otherwise it returns `Ok(())` discarding `cap`.
	///
	/// # example
	/// ```
	/// assert_eq!(Capture::ok(|| 1), Ok(1));
	/// assert_eq!(Check::ok(|| 1), Ok(()));
	/// ```
	fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self>;

	/// transform the result of `err` into [`Err`] based on [`error`](#error-feature) feature.
	///
	/// it returns `Err(err()) if `error` is enabled, otherwise it returns `Err(())` discarding `err`.
	///
	/// # example
	/// ```
	/// let err = MatchError::mismatch(Expected::SomeThing, 0);
	/// assert_eq!(Parse::err::<()>(|| err.clone()), Err(err));
	/// assert_eq!(Test::err::<()>(|| err.clone()), Err(()));
	/// ```
	fn err<T>(err: impl FnOnce() -> MatchError) -> Result<T, Self::Error>;

	/// transform `val` into [`Success<T>`](Self::Success) based on [`capture`](#capture-feature) feature.
	///
	/// it returns `val` if `capture` is enabled, otherwise it returns `()` discarding `val`.
	///
	/// # example
	/// ```
	/// assert_eq!(Capture::wrap_success(1), 1);
	/// assert_eq!(Check::wrap_success(1), ());
	/// ```
	fn wrap_success<T>(val: T) -> Self::Success<T>;

	/// transform `err` into [`Error`](Self::Error) based on [`error`](#error-feature) feature.
	///
	/// it returns `err` if `error` is enabled, otherwise it returns `()` discarding `err`.
	///
	/// # example
	/// ```
	/// let err = MatchError::mismatch(Expected::SomeThing, 0);
	/// assert_eq!(Parse::wrap_error(err.clone()), err);
	/// assert_eq!(Test::wrap_error(err.clone()), ());
	/// ```
	fn wrap_error(err: MatchError) -> Self::Error;

	/// transform [`Success<T>`](Self::Success) into `T` based on [`capture`](#capture-feature) feature.
	///
	/// it returns `val` if `capture` is enabled, otherwise it panics.
	///
	/// # example
	/// ```
	/// assert_eq!(Capture::unwrap_success(Capture::wrap_success(1)), 1);
	/// // this panics
	/// Check::unwrap_success(1);
	/// ```
	#[inline]
	fn unwrap_success<T>(val: Self::Success<T>) -> T {
		let _ = val;
		panic!("unwrap_success called on no-capture mode")
	}

	/// transform [`Error`](Self::Error) into [`MatchError`] based on [`error`](#error-feature) feature.
	///
	/// it returns `val` if `error` is enabled, otherwise it panics.
	///
	/// # example
	/// ```
	/// let err = MatchError::mismatch(Expected::SomeThing, 0);
	/// assert_eq!(Check::unwrap_error(Check::wrap_error(err.clone())), err);
	/// // this panics
	/// Test::unwrap_error(err);
	/// ```
	#[inline]
	fn unwrap_error(val: Self::Error) -> MatchError {
		let _ = val;
		panic!("unwrap_error called on no-error mode")
	}

	/// map [`Success<T>`](Self::Success) into `Success<U>` by applying `fun` on its wrapped value.
	///
	/// it return `fun(val)` if [`capture`](#capture-feature) is enabled, otherwise it returns `()`.
	///
	/// # example
	/// ```
	/// assert_eq!(Capture::map(Capture::wrap_success(1), |x| x + 1), 2);
	/// assert_eq!(Check::map(Capture::wrap_success(1), |x| x + 1), ());
	/// ```
	fn map<T, U>(val: Self::Success<T>, fun: impl FnOnce(T) -> U) -> Self::Success<U>;
}

/// generate concrete [`Mode`].
macro_rules! decl_mode {
		(
			$(#[$attr:meta])*
			$name:ident {
				$(capture: true $cap_true:vis)? $(capture: false $cap_false:vis)?,
				$(error: true $err_true:vis)? $(error: false $err_false:vis)?,
				+cap -> $plus_cap:ty,
				+err -> $plus_err:ty,
				-cap -> $minus_cap:ty,
				-err -> $minus_err:ty,
			}
		) => {
			$(#[$attr])*
			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub struct $name;
			impl Mode for $name {
				$($cap_true type Success<T> = T;)?
				$($cap_false type Success<T> = ();)?
				$($err_true type Error = MatchError;)?
				$($err_false type Error = ();)?

				$($cap_true const DO_CAPTURE: bool = true;)?
				$($cap_false const DO_CAPTURE: bool = false;)?
				$($err_true const DO_ERROR: bool = true;)?
				$($err_false const DO_ERROR: bool = false;)?

				type WithCapture = $plus_cap;
				type WithError = $plus_err;
				type WithoutCapture = $minus_cap;
				type WithoutError = $minus_err;

				#[inline]
				$($cap_true fn ok<T>(cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(cap())
				})?
				$($cap_false fn ok<T>(_cap: impl FnOnce() -> T) -> MatchResult<T, Self> {
					Ok(())
				})?
				#[inline]
				$($err_true fn err<T>(err: impl FnOnce() -> MatchError) -> Result<T, Self::Error> {
					Err(err())
				})?
				$($err_false fn err<T>(_err: impl FnOnce() -> MatchError) -> Result<T, Self::Error> {
					Err(())
				})?

				#[inline]
				$($cap_true fn wrap_success<T>(val: T) -> Self::Success<T> { val })?
				$($cap_false fn wrap_success<T>(_val: T) -> Self::Success<T> {  })?
				#[inline]
				$($err_true fn wrap_error(err: MatchError) -> Self::Error { err })?
				$($err_false fn wrap_error(_err: MatchError) -> Self::Error {  })?


				$(#[inline] $cap_true fn unwrap_success<T>(val: Self::Success<T>) -> T { val })?
				$(#[inline] $err_true fn unwrap_error(val: Self::Error) -> MatchError { val })?

				#[inline]
				$($cap_true fn map<T, U>(val: Self::Success<T>, fun: impl FnOnce(T) -> U)
					-> Self::Success<U> { fun(val) }
				)?
				$($cap_false fn map<T, U>(_: Self::Success<T>, _: impl FnOnce(T) -> U)
					-> Self::Success<U> {  }
				)?
			}
		};
	}
decl_mode!(
	/// concrete [`Mode`] with no feature enabled.
	///
	/// in `Test`: [`capture`](Mode#capture-feature) is `false`, [`error`](Mode#error-feature) is `false`, its simplified result is `bool`.
	///
	/// it monomorphize to the lightweightest matching logic, great for test based peeks and "did match" tests.
	///
	/// # example
	/// ```
	/// assert_eq!("abc".do_match::<Test>("abc", &mut 0), Ok(()));
	/// assert_eq!("abc".do_match::<Test>("abd", &mut 0), Err(()));
	///
	/// let value = "abc";
	/// let mut off = 0;
	/// if "a".test(value, &mut off) {
	///     "bc".check(value, &mut off)?;
	/// }
	/// ```
	Test {
		capture: false, error: false,
		+cap -> Capture, +err -> Check,
		-cap -> Test, -err -> Test,
	}
);
decl_mode!(
	/// concrete [`Mode`] with [`error`](Mode#error-feature) feature enabled.
	///
	/// in `Check`: [`capture`](Mode#capture-feature) is `false`, [`error`](Mode#error-feature) is `true`, and its simplified result is `Result<(), MatchError>`.
	///
	/// it is great for `did match` checks with detailed [`MatchError`]s.
	///
	/// # example
	/// ```
	/// assert_eq!("abc".do_match::<Check>("abc", &mut 0), Ok(()));
	/// assert_eq!(
	///     "abc".do_match::<Check>("abd", &mut 0),
	///     Err(MatchError::mismatch(Expected::A("\"abc\"".into()), 0))
	/// );
	/// ```
	Check {
		capture: false, error: true,
		+cap -> Parse, +err -> Check,
		-cap -> Check, -err -> Test,
	}
);
decl_mode!(
	/// concrete [`Mode`] with [`capture`](Mode#capture-feature) feature enabled.
	///
	/// in `Capture`: [`capture`](Mode#capture-feature) is `true`, [`error`](Mode#error-feature) is `false`, and its simplified result is `Option<T>`.
	///
	/// it is great for grammar based extractions and data transformation that doesnt need detailed [`MatchError`]s.
	///
	/// # example
	/// ```
	/// assert_eq!("abc".do_match::<Capture>("abc", &mut 0), Ok("abc"));
	/// assert_eq!("abc".do_match::<Capture>("abd", &mut 0), Err(()));
	///
	/// let (name, value) = try_match!("abc = 123",
	///     (name = alphanum+) ws* '=' ws* (value = dec+ => nb.parse::<u64>().unwrap())
	/// ).unwrap();
	/// assert_eq!((name, value), ("abc", 123));
	/// ```
	Capture {
		capture: true, error: false,
		+cap -> Capture, +err -> Parse,
		-cap -> Test, -err -> Capture,
	}
);
decl_mode!(
	/// concrete [`Mode`] with all features enabled.
	///
	/// in `Parse`: [`capture`](Mode#capture-feature) is `true`, [`error`](Mode#error-feature) is `true`, and its simplified result is `Result<T, MatchError>`.
	///
	/// it is great for advance parsers that need all features of [`Matcher`]s, both the expressive captures and detailed [`MatchError`]s.
	///
	/// # example
	/// ```
	/// assert_eq!("abc".do_match::<Parse>("abc", &mut 0), Ok("abc"));
	/// assert_eq!(
	///     "abc".do_match::<Parse>("abd", &mut 0),
	///     Err(MatchError::mismatch(Expected::A("\"abc\"".into()), 0))
	/// );
	///
	/// let matcher = matcher!(for str:
	///     (name = alphanum+) ws* '=' ws* (value: u64 = dec+ => nb.parse().unwrap())
	/// );
	/// assert_eq!(parse("abc = 123", matcher), Ok(("abc", 123)));
	/// assert_eq!(
	///     parse("abc = ", matcher),
	///     Err(MatchError::incomplete(Expected::A("a decimal digit".into()), 6))
	/// );
	/// ```
	Parse {
		capture: true, error: true,
		+cap -> Parse, +err -> Parse,
		-cap -> Check, -err -> Capture,
	}
);

/// a type that can match a [`MatchAble`].
///
/// the `Matcher` trait enable a type to behaive like a pattern for a specific [`MatchAble`].
///
/// `Matcher`s doesnt just return bool as the match result, they can produce a [`Capture`](Self::Capture) and produce detailed [`MatchError`], all controlled through [`Mode`].
///
/// # implementation guide
///
/// `Matcher`s are advice to be universal, pure, primitive, and efficient, as `Matcher` is a universal trait used inside the quickest tester and the advanced parsers.
///
/// the matching logic is hosted inside `do_match`, it is generic over [`Mode`], take the [`MatchAble`] and an offset, and return a [`MatchResult`].
///
/// the offset is a token aligned, can be forward if needed, must remain token aligned on success, but it can be anything on failure.
///
/// as a primitive unit, the `do_match` should contain primitive matching logic and offset handling, and be generic over [`Mode`] through its selectors and guards.
///
/// ## example
/// ```
/// impl Matcher<Tokens> for Token {
///     type Capture<'src> = Token where T: 'src;
///     fn do_match<'src, M: Mode>(
///         &self, matched: &'src Tokens, off: &mut usize,
///     ) -> MatchResult<Self::Capture<'src>, M> {
///         if let Some(token) = matched.get_token(*off) && token == *self {
///             *off += 1;
///             M::ok(|| token)
///         } else {
///             M::err(|| MatchError::expected(
///                 self.expected(), *off == matched.len(), *off
///             ))
///         }
///     }
///
///     fn expected(&self) -> Expected {
///         Expected::A(format!("{self:?}").into())
///     }
/// }
/// ```
///
/// # builtin universal `Matchers`
/// some common types implementes `Matcher` for any [`MatchAble`] type, supports `T: Matcher`.
/// - `()`: acts as noop and always match with `()`.
/// - `&T`, [`Box<T>`], [`Rc<T>`], [`Arc<T>`]: redirect matching to `T`.
/// - `Option<T>`: if it is `Some(T)`, it matches by `T` with `Some(T::Capture)`, else it acts as noop and always matches with `None`.
///
/// ### example
/// ```
/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
/// let mut off = 0;
///
/// assert_eq!(().parse(&tokens, &mut off), Ok(()));
/// assert_eq!(off, 0);
///
/// assert_eq!((&&Token::Nb(1)).parse(&tokens, &mut off), Ok(Token::Nb(1)));
/// assert_eq!(Box::new(Token::Add).parse(&tokens, &mut off), Ok(Token::Nb(1)));
///
/// assert_eq!(None::<Token>.parse(&tokens, &mut off), Ok(None));
/// assert_eq!(off, 2);
/// assert_eq!(Some(Token::Nb(2)).parse(&tokens, &mut off), Ok(Some(Token::Nb(2))));
/// ```
pub trait Matcher<T: MatchAble + ?Sized> {
	/// the match product of the `Matcher`.
	///
	/// it can be any type, but usually [`MatchAble::Slice`].
	///
	/// it must be linked to the lifetime of the matchable through `'src` lifetime.
	type Capture<'src>
	where
		T: 'src;

	/// the matching logic of the `Matcher`.
	///
	/// see the [implementation guide](#implementation-guide) for more info.
	///
	/// this is a low level function, for general use check the high level items.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	///
	/// assert_eq!(Token::Nb(1).do_match::<Parse>(&tokens, &mut off), Ok(Token::Nb(1)));
	/// assert_eq!(off, 1);
	///
	/// assert_eq!(
	///     Token::Nb(1).do_match::<Check>(&tokens, &mut off),
	///     Err(MatchError::mismatch(Expected::A("Nb(1)".into()), 1))
	/// );
	/// assert_eq!(off, 1);
	///
	/// assert_eq!(Token::Add.do_match::<Test>(&tokens, &mut off), Ok(()));
	/// ```
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<Self::Capture<'src>, M>;

	/// [`do_match`](Self::do_match) shorthand for [`Test`] [`Mode`].
	///
	/// the return type is `bool` where `Ok(())` is `true` and `Err(())` is `false`.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	///
	/// assert!(Token::Nb(1).test(&tokens, &mut off));
	/// assert_eq!(off, 1);
	/// assert!(!Token::Nb(1).test(&tokens, &mut off));
	/// ```
	#[inline]
	fn test(&self, matched: &T, off: &mut usize) -> bool {
		self.do_match::<Test>(matched, off).is_ok()
	}

	/// [`do_match`](Self::do_match) shorthand for [`Check`] [`Mode`].
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	///
	/// assert_eq!(Token::Nb(1).check(&tokens, &mut off), Ok(()));
	/// assert_eq!(off, 1);
	///
	/// assert_eq!(
	///     Token::Nb(1).check(&tokens, &mut off),
	///     Err(MatchError::mismatch(Expected::A("Nb(1)".into()), 1))
	/// );
	/// ```
	#[inline]
	fn check(&self, matched: &T, off: &mut usize) -> Result<(), MatchError> {
		self.do_match::<Check>(matched, off)
	}

	/// [`do_match`](Self::do_match) shorthand for [`Capture`] [`Mode`].
	///
	/// the return type is `Option<Capture>` where `Ok(cap)` is `Some(cap)` and `Err(())` is `None`.
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	///
	/// assert_eq!(Token::Nb(1).capture(&tokens, &mut off), Some(Token::Nb(1)));
	/// assert_eq!(off, 1);
	///
	/// assert_eq!(Token::Nb(1).capture(&tokens, &mut off), None);
	/// ```
	#[inline]
	fn capture<'src>(
		&self, matched: &'src T, off: &mut usize,
	) -> Option<Self::Capture<'src>> {
		self.do_match::<Capture>(matched, off).ok()
	}

	/// [`do_match`](Self::do_match) shorthand for [`Parse`] [`Mode`].
	///
	/// # example
	/// ```
	/// let tokens = Tokens(&[Token::Nb(1), Token::Add, Token::Nb(2)]);
	/// let mut off = 0;
	///
	/// assert_eq!(Token::Nb(1).parse(&tokens, &mut off), Ok(Token::Nb(1)));
	/// assert_eq!(off, 1);
	///
	/// assert_eq!(
	///     Token::Nb(1).parse(&tokens, &mut off),
	///     Err(MatchError::mismatch(Expected::A("Nb(1)".into()), 1))
	/// );
	/// ```
	#[inline]
	fn parse<'src>(
		&self, matched: &'src T, off: &mut usize,
	) -> Result<Self::Capture<'src>, MatchError> {
		self.do_match::<Parse>(matched, off)
	}

	/// return what the `Matcher` expect to match as beginning.
	///
	/// the default implementation returns [`Expected::None`].
	///
	/// # example
	/// ```
	/// assert_eq!(Token::Eq.expected(), Expected::A("Eq".into()));
	/// assert_eq!(
	///     matcher!(for str: 'a' | 'b' | 'c').expected(),
	///     Expected::OneOf(vec!["\"a\"".into(), "\"b\"".into(), "\"c\"".into()])
	/// );
	/// ```
	fn expected(&self) -> Expected {
		Expected::None
	}
}

impl<T: MatchAble + ?Sized> Matcher<T> for () {
	type Capture<'src>
		= ()
	where
		T: 'src;
	fn do_match<M: Mode>(&self, _matched: &T, _off: &mut usize) -> MatchResult<(), M> {
		Ok(M::wrap_success(()))
	}
}
impl<T: MatchAble + ?Sized, U: Matcher<T> + ?Sized> Matcher<T> for &U {
	type Capture<'src>
		= U::Capture<'src>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, matched: &'src T, off: &mut usize,
	) -> MatchResult<U::Capture<'src>, M> {
		(*self).do_match::<M>(matched, off)
	}
	fn expected(&self) -> Expected {
		(*self).expected()
	}
}
macro_rules! as_ref_matcher {
	[$($ty:ident),+] => {
		$(impl<T: MatchAble + ?Sized, U: Matcher<T> + ?Sized> Matcher<T> for $ty<U> {
			type Capture<'src> = U::Capture<'src> where T: 'src;
			fn do_match<'src, M: Mode>(
				&self, matched: &'src T, off: &mut usize,
			) -> MatchResult<U::Capture<'src>, M> {
				AsRef::<U>::as_ref(self).do_match::<M>(matched, off)
			}
			fn expected(&self) -> Expected {
				AsRef::<U>::as_ref(self).expected()
			}
		})+
	};
}
as_ref_matcher![Box, Rc, Arc];
impl<T: MatchAble + ?Sized, U: Matcher<T>> Matcher<T> for Option<U> {
	type Capture<'src>
		= Option<U::Capture<'src>>
	where
		T: 'src;
	fn do_match<'src, M: Mode>(
		&self, value: &'src T, off: &mut usize,
	) -> MatchResult<Option<U::Capture<'src>>, M> {
		match self {
			Some(matcher) => {
				matcher.do_match::<M>(value, off).map(|suc| M::map(suc, Some))
			}
			None => Ok(M::wrap_success(None)),
		}
	}
}

fn match_no_excess<T: MatchAble + ?Sized, U: Matcher<T>, M: Mode>(
	value: &T, matcher: U,
) -> MatchResult<U::Capture<'_>, M> {
	let mut off = 0;
	let res = matcher.do_match::<M>(value, &mut off);
	if off == value.len() || res.is_err() {
		res
	} else {
		M::err(|| MatchError::excess(off))
	}
}
pub fn matches<T: MatchAble + ?Sized>(value: &T, matcher: impl Matcher<T>) -> bool {
	match_no_excess::<_, _, Test>(value, matcher).is_ok()
}
pub fn check<T: MatchAble + ?Sized>(
	value: &T, matcher: impl Matcher<T>,
) -> Result<(), MatchError> {
	match_no_excess::<_, _, Check>(value, matcher)
}
pub fn try_match<T: MatchAble + ?Sized, U: Matcher<T>>(
	value: &T, matcher: U,
) -> Option<U::Capture<'_>> {
	match_no_excess::<_, _, Capture>(value, matcher).ok()
}
pub fn parse<T: MatchAble + ?Sized, U: Matcher<T>>(
	value: &T, matcher: U,
) -> Result<U::Capture<'_>, MatchError> {
	match_no_excess::<_, _, Parse>(value, matcher)
}
