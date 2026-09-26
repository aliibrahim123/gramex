/// define a grammar declaration.
///
/// the `grammar` macro generate matchers with their [generated items](gram_ref#generated-items) from a set of [terms](#term).
///
/// ```gramex
/// let body = "for" matched:type ';' terms*:term;
/// ```
/// `matched` is the [`MatchAble`] type the terms target.
///
/// # term
/// ```gramex
/// let term = "let" name:ident args?:list<ident, ','> (':' type:type)?
///     '=' expr ("=>" map:rust_expr)? ';';
/// ```
///
/// a term is a [grammar expression](gram_ref) identified by a `name` that get transformed into a [`Matcher`] that matches by this expression.
///
/// ```
/// grammar! {
///     for str;
///     let ident = ('a'..'z' | 'A'..'Z' | '0'..'9')+;
///     let nb = ('0'..'9')+;
/// }
/// assert!(matches("123", nb));
/// assert!(matches("Abc123", ident));
/// assert!(!matches("$abc", ident));
/// ```
///
/// a term has an implicit root [capture](gram_ref#captures) having its name, its resolved type is used as the [`Matcher`] [`Capture`](Matcher::Capture).
///
/// the [type specifier](gram_ref#type-specifier) and the [map](gram_ref#mapping) are optional parts that get applied to the root capture.
///
/// ```
/// grammar! {
///     for str;
///     let ident = ('a'..'z' | 'A'..'Z' | '0'..'9')+;
///     let nb: i64 = '-'? ('0'..'9')+ => nb.parse().unwrap();
///     let val: enum = true:"true" | false:"false" | ident:ident | nb:nb | arr:arr;
/// }
///
/// assert_eq!(try_match("abc", ident), Some("abc"));
/// assert_eq!(try_match("-123", nb), Some(-123));
/// assert_eq!(try_match("abc", val), Some(value::Ident("abc")));
/// ```
///
/// `args` are optional [`Matcher`] arguments that get binded to the expression and transform the generated [`Matcher`] into [compound matcher](gram_ref#call-atom).
///
/// ```
/// grammar! {
///     for str;
///     let list<item, sep>: Vec<Item::Capture<'src>> =
///         first:item (items* = sep item:item => item)
///         => std::iter::chain(Some(first), items).collect();
/// }
/// assert_eq!(parse("a,b,c", list(ident, ',')), Ok(vec!["a", "b", "c"]));
/// assert_eq!(parse("1+2+3", list(nb, '+')), Ok(vec![1, 2, 3]));
/// ```
pub use gramex_macro::grammar;

/// generate an ananymous [`Matcher`] from a [grammar expression](gram_ref).
///
/// ```gramex
/// let body = "for" matched:type ',' ("Capture" ':' type)? expr ("=>" map:rust_expr)?
/// ```
///
/// `matched` is the [`MatchAble`] type and `expr` is the [grammar expression](gram_ref).
///
/// the generated item is a [`Copy`] value implementing [`Matcher`].
///
/// ```
/// let ident = matcher!(for str, ('a'..'z' | 'A'..'Z' | '0'..'9')+);
/// assert!(matches("abc", ident));
/// assert!(parse("$abc", ident).is_err());
/// ```
///
/// the expression is inside an implicit root [capture](gram_ref#captures) named `root` whose resolved type is used as the [`Matcher`] [`Capture`](Matcher::Capture).
///
/// the `Capture` [type specifier](gram_ref#type-specifier) and the [map](gram_ref#mapping) are optional parts that get applied to the root capture.
///
/// ```
/// let nb = matcher!(for str, Capture: i64, '-'? ('0'..'9')+ => root.parse().unwrap());
/// assert_eq!(try_match("-123", nb), Some(-123));
/// assert_eq!(try_match("123", nb), Some(123));
/// ```
pub use gramex_macro::matcher;

/// fully match a value by a [grammar expression](gram_ref), [`Check`](crate::modes::Check) [`Mode`].
///
/// ```gramex
/// let body = ("for" matched:type ',')? value:rust_expr ',' expr;
/// ```
///
/// `check` takes a [`MatchAble`] value and optionally its type in `matched`, and the grammer expression `expr`, `expr` can access local variables.
///
/// it return `Result<(), MatchError>` for if the expression matches the entire `value` from offset `0` till `value.len()`. it return `false` if there are excess input.
///  
/// # example
/// ```
/// let b = 'b';
/// assert!(matches!("abc", 'a' b? !'d'+));
/// assert!(!matches!("abd", 'a' b? !'d'+));
/// assert!(!matches!(for str, "abcd", 'a' b? !'d'+));
/// ```
pub use gramex_macro::check;

/// fully match a value by a [grammar expression](gram_ref), [`Test`](crate::modes::Test) [`Mode`].
///
/// ```gramex
/// let body = ("for" matched:type ',')? value:rust_expr ',' expr;
/// ```
///
/// `matches` takes a [`MatchAble`] value and optionally its type in `matched`, and the grammer expression `expr`, `expr` can access local variables.
///
/// it return `bool` for if the expression matches the entire `value` from offset `0` till `value.len()`. it return [excess](crate::result::MatchErrorKind::Excess) [`MatchError`] if there are excess input.
///  
/// # example
/// ```
/// let b = 'b';
/// assert_eq!(check!("abc", 'a' b? !'d'+), Ok(()));
/// assert!(!check!("abd", 'a' b? !'d'+).is_err());
/// assert_eq!(!check!(for str, "abcd", 'a' b? !'d'+), Err(MatchError::excess(3)));
/// ```
pub use gramex_macro::matches;

/// fully match a value by a [grammar expression](gram_ref), [`Parse`](crate::modes::Parse) [`Mode`].
///
/// ```gramex
/// let body = ("for" matched:type ',')? value:rust_expr ',' expr;
/// ```
///
/// `parse` takes a [`MatchAble`] value and optionally its type in `matched`, and the grammer expression `expr`, `expr` can access local variables.
///
/// it return `Result<Capture, MatchError>` for if the expression matches the entire `value` from offset `0` till `value.len()`. it return [excess](crate::result::MatchErrorKind::Excess) [`MatchError`] if there are excess input.
///
/// the expression is inside an implicit root [capture](gram_ref#captures) whose result is used as the return type.
///
/// # example
/// ```
/// let b = 'b';
/// assert_eq!(parse!("abc", a:'a' b? (c = !'d'+)), Ok(("a", "c")));
/// assert!(parse!("abd", a:'a' b? (c = !'d'+)).is_err());
/// assert_eq!(parse!("abcd", a:'a' b? (c = !'d'+)), Err(MatchError::excess(3)));
/// ```
pub use gramex_macro::parse;

/// fully match a value by a [grammar expression](gram_ref), [`Capture`](crate::modes::Capture) [`Mode`].
///
/// ```gramex
/// let body = ("for" matched:type ',')? value:rust_expr ',' expr;
/// ```
///
/// `try_match` takes a [`MatchAble`] value and optionally its type in `matched`, and the grammer expression `expr`, `expr` can access local variables.
///
/// it return `Option<Capture>` for if the expression matches the entire `value` from offset `0` till `value.len()`. it return `None` if there are excess input.
///
/// the expression is inside an implicit root [capture](gram_ref#captures) whose result is used as the return type.
///
/// # example
/// ```
/// let b = 'b';
/// assert_eq!(try_match!("abc", a:'a' b? (c = !'d'+)), Some(("a", "c")));
/// assert_eq!(try_match!("abd", a:'a' b? (c = !'d'+)), None);
/// assert_eq!(try_match!("abcd", a:'a' b? (c = !'d'+)), None);
/// ```
pub use gramex_macro::try_match;

#[allow(unused)]
use crate::{result::*, *};
