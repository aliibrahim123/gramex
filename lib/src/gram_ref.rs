//! # grammmar reference
//! the grammar expressions is a matching construct / DSL inspired by the typical metasyntax languages with a rust flavor.
//!
//! these expressions work on all [`MatchAble`] types, and they get transformed into efficient [raw `Matcher`](Matcher#impl) style matching logic.
//!
//! they can be used in different contexes, as standalone items or inlined with the code, by the different macros provided by gramex: [`gramex!`], [`matches!`], [`try_match!`], [`check!`], [`parse!`], [`matcher!`], [`eat!`], [`try_eat!`], [`test!`], and [`match_map!`].
//!
//! the grammar expressions are composed of [atoms](#atoms) representing atomic patterns, and expressions chains representing compound patterns.
//!
//! ```gramex
//! let expr = unit | seq | and | or | imply | capture;
//! ```
//!
//! # atoms
//! ```gramex
//! let atom = any_atom | value_atom | range_atom | call_atom | group_atom;
//! ```
//! atoms are the most primitive unit of the grammar, they define atomic patterns that can not be further separated in the expression chain.
//!
//! ### any
//! ```gramex
//! let any_atom = '_';
//! ```
//! the any atom (`_`) matches exactly 1 [token](MatchAble::Token), and mostly fail due to incomplete input.
//!
//! under the hode, it calls [`MatchAble::skip_n(n: 1)`](MatchAble::skip_n) on the matched type.
//!
//! ```
//! assert!(matches!("a", _));
//! assert!(!matches!("", _));
//! ```
//!
//! ### value atoms
//! ```gramex
//! let value_atom = lit | path | expr_block;
//! ```
//! value atoms match against a [`Matcher`] resolved by the tokens defining it.
//!
//! these tokens can be literials, [simple paths](https://doc.rust-lang.org/reference/paths.html#simple-paths) including identifiers, and expression blocks that get inserted directly in the generated code.
//!
//! the [`Matcher`] is called with only the required [`Mode` features](Mode#features), and its error is directly propagated.
//!
//! ```rust
//! assert!(matches!("a", 'a'));
//! mod consts {
//!  const B: char = 'b';
//! }
//! assert!(matches!("b", consts::b));
//! assert!(matches!("c", { a(|v| v == 'c') }));
//! ```
//!
//! ### range atom
//! ```gramex
//! let range_atom = value_atom ".." value_atom;
//! ```
//! range atom match against a range of values defined by 2 [value atoms](#value-atoms).
//!
//! the bounds that can be of any atom type but must resolve to the same underlying type.
//!
//! range atom get transformed into a [`RangeInclusive`] that must implement [`Matcher`].
//!
//! ```
//! assert!(matches!("a", 'a'..'z'));
//! const Z: &str = "Z"
//! assert!(!matches!("1",  "A"..Z));
//! ```
//!
//! ### call atom
//! ```gramex
//! let matcher = ("for" matched_type:ty)? expr ("=>" rust_expr)?;
//! let call_atom = path "<" args:list<matcher, ','> ">";
//! ```
//! the call atom matches a compound matcher by a set of argument.
//!
//! a compound matcher is a `Fn(..Matcher) -> Matcher`, the passed arguments are transformed into ananymous [`Matcher`]s and passed to the function, then its return [`Matcher`] is matched against.
//!
//! the arguments are matcher declerations with a root expression, an optional matched type specifier (default to the current one), and an optional [map](#mapping) for the root [capture](#captures).
//!
//! ```rust
//! assert!(matches!("ababa", list<"a", "b">));
//! ```
//!
//! ### group atom
//! ```gramex
//! let group_atom = '(' expr ')';
//! ```
//! the group expression is a grammer expression enclosed by paranthesis.
//!
//! ```
//! assert!(matches!("1", !('a'..'z')));
//! ```
//!
//! # units
//! ```gramex
//! let unit = not?:'!' near?:'~' atom rep?;
//! ```
//! units are [atoms](#atoms) with optional modifications on top.
//!
//! the [range atom](#range-atom) can not have these modifications.
//!
//! ### not operator
//! the not operator `!` matches exactly one [token](MatchAble::Token) if the [atom](#atoms) doesnt match.
//!
//! the not operator also fail at incomplete input.
//!
//! ```
//! assert!(matches!("a", !"bb"));
//! assert!(!matches!("b", !"b"));
//! assert!(!matches!("", !"b"));
//! ```
//!
//! ### near operator
//! the near operator `~` matches the [atom](#atoms) without advancing.
//!
//! can be combined with not for inverted result, incomplete input are positive.
//!  
//! ```
//! assert!(matches!("a", ~'a' _));
//! assert!(matches!("a", !~'b' _));
//! assert!(matches!("", !~'b' _));
//! ```
//!
//! ## repetitions
//! ```gramex
//! let rep = opt:'?' | multi0:'*' | multi1:'+' | (exact = '[' count:nb ']') |
//!  (range: '[' min?:nb ".." max?:nb ']');
//! ```
//!
//! repetitions matches an [atom](#atoms) some interval of times.
//!
//! #### shorthands
//! `?` match the [atom](#atoms) 0 or 1 times.
//! ```
//! assert!(matches!("a", "a"?));
//! assert!(matches!("", "a"?));
//! ```
//!
//! `*` match the [atom](#atoms) 0 or more times.
//! ```
//! assert!(matches!("aa", "a"*));
//! assert!(matches!("", "a"*));
//! ```
//!
//! `+` match the [atom](#atoms) 1 or more times.
//! ```
//! assert!(matches!("aa", "a"*));
//! assert!(!matches!("", "a"*));
//! ```
//!
//! `[n]` match the [atom](#atoms) exactly `n` times.
//! ```
//! assert!(matches!("aa", "a"[2]));
//! assert!(!matches!("a", "a"[2]));
//! ```
//!
//! #### canocical form
//! `[min..max]` match the [atom](#atoms) between `min` and `max` (inclusive) times.
//!
//! `min` and `max` are optional and default to `0` and infinity respectifily.
//! ```
//! assert!(matches!("aaa", "a"[2..4]));
//! assert!(!matches!("a", "a"[2..4]));
//! assert!(matches!("a", "a"[..4]));
//! assert!(matches!("aaa", "a"[2..]));
//! ```
//!
//! #### notes
//! unbounded repetition is greedy eating till mismatch or end of input, and bounded one stop instantly when hitting max.
//! ```
//! assert!(matches!("aaab", "a"+ "b"));
//! assert!(!matches!("aaaa", "a"+ "a"));
//! assert!(matches!("aaaa", "a"[3] "a"));
//! ```
//!
//! with [not](#not-operator), the repetition takes precedence, while with [near](#near-operator) and its negated form, it take precedence.
//!
//! ```
//! matches!("abc", !"d"[3]) // <=> matches!("abc", (!"d")[3])
//! matches!("aaa", ~"a"[3] _*) // <=> matches!("aaa", ~("a"[3]) _*)
//! ```
//!
//! # flow expressions
//! flow expressions are expressions that apply a flow on top of other expressions.
//!
//! the precedence of these expressions is: [`unit`](#units) / [`capture`](#captures) > [`and`](#and-expression) > [`seq`](#sequence-expression) > [`imply`](#imply-expression) > [`or`](#or-expression).
//!
//! ## sequence expression
//! ```gramex
//! let seq = expr+;
//! ```
//! the sequence expression match multiple expressions separated by whitespaces in order.
//!
//! ```
//! assert!(matches!("abc", "a" "b"? !"d"));
//! assert!(!matches!("abd", "a" "b"? !"d"));
//! ```
//!
//! ## or expression
//! ```gramex
//! let or = list<expr, '|'>;
//! ```
//! the or expression matches one of multiple expressions separated by `|`.
//!
//! the expressions get matched in order with the same start and the first match win.
//!
//! if an expression is an [imply](#imply-expression) and its condition matched, the or propagate the result of its then expression instead of matching the rest of the expressions.
//!
//! ```
//! assert!(matches!("b", 'a' | 'b' | 'c'));
//! assert!(!matches!("d", 'a' | 'b' | 'c'));
//! assert!(!matches!("ab", 'a' | "ab"));
//! assert!(!matches!("bb", 'a' | "b" -> 'a' | "bb"));
//! ```
//!
//! ## and expression
//! ```gramex
//! let and = list<expr, '&'>;
//! ```
//! the and expression matches all of multiple expressions separated by `&` with the same section.
//!
//! the and expression first matches its first expression, then it [slices](MatchAble::slice) the input till where the first end, then it match the rest on it in order with the same start.
//!
//! the and expression requires [`MatchAble::Slice`] to be [`MatchAble`].
//!
//! ```
//! assert!(matches!("abc", _[3] & 'a' & _ !'a'+));
//! assert!(!matches!("ab", _[3] & 'a' & _ !'a'+));
//! assert!(!matches!("bbc", _[3] & 'a' & _ !'a'+));
//! ```
//!
//! ## imply expression
//! ```gramex
//! let imply = cond:expr "->" then:expr;
//! ```
//! the imply expression express implication logical relation between two expressions.
//!
//! if the condition matches, it match the then expression after it, else it match nothing.
//!
//! ```
//! assert!(matches!("ab", 'a' -> 'b'));
//! assert!(!matches!("ac", 'a' -> 'b'));
//! assert!(matches!("", 'a' -> 'b'));
//! ```
//!
//! # captures
//! ```gramex
//! let capture = name:ident rep? ':' (atom & !group_atom) |
//!     (name:ident rep? (":" type:cap_type)? '=' expr ("=>" map:rust_expr)?);
//! ```
//! the capture matches an expression then extract the matched section as a result.
//!
//! there are 2 syntax for captures, a shorthand one with an [atom](#atoms) `name:atom` and a full version with an expression `(name = expr)`.
//!
//! ```
//! assert_eq!(try_match!("abc", 'a' bc:"bc"), Some(("bc",)));
//! assert_eq!(try_match!("abc", 'a' (bc = !('0'..'9') 'c')), Some(("bc",)));
//! ```
//!
//! captures are allowed in most cases, but are forbidden in [negated units](#not-operator), non optional specified [repetition](#repetitions) units, arguments of non captured [call atoms](#call-atom), and non capturing macros like [`matches!`].
//!
//! ```
//! try_match!("", !(_ not_allowed1:_) (_ not_allowed2:_)* list(not_allowed3:_, ','));
//! matches!("", not_allowed4:_);
//! assert_eq!(try_match!("a", (_ & (~(~_ allowed:_) -> _) | _)?), Some((Some("a"),)));
//! ```
//!
//! ### repetition
//! captures support [repetition](#repetitions), either by infering it from the optionality of the path ([optional](#repetitions) units, [or](#or-expression) and [imply](#imply-expression) expressions), or by manually specifing it.
//!
//! repetition override the capture resolved type, it become [`Option<T>`] if repetition is optional, and [`Vec<T>`] otherwise.
//!
//! ```
//! assert_eq!(try_match!("a", a?:'a'), Some((Some("a"),)));
//! assert_eq!(try_match!("a", (_ a:'a')?), Some((None,)));
//! assert_eq!(try_match!("aaa", (_ a[1..3]:'a')?), Some((vec!["a", "a"],)));
//! ```
//!
//! ### type specifier
//! ```gramex
//! let cap_type = type | "struct" ident? | "enum" ident?;
//! ```
//! the capture type specifier specifies the base type of the capture (without the [repetition](#repetition)) when requried.
//!
//! captures doesnt always require to specify its type, only when the default type doesnt work in contexes requiring it: [terms](gramex!#term) and matchers root capture, [generated items](#generated-items) nested captures.
//!
//! inside the type specifier, a `'src` lifetime is given related to the [matched value](MatchAble) lifetime.
//!
//! ```
//! assert_eq!(
//!     try_match!("abc", a:'a' (bc = "bc" => bc.to_string())),
//!     Some(("a", "bc")),
//! );
//! gramex! {
//!     for str;
//!     let nb: u8 = ('0'..'9')+ => nb.parse().unwrap();
//! }
//! ```
//!
//! ### mapping
//! mapping is an optional rust expression that transform the matched section into a different value / type.
//!
//! it is specified though `=> expr` after the expression, and the matched section is binded as the capture name.
//!
//! note that a [type specifier](#type-specifier) may be needed when using it.
//!
//! ```
//! assert_eq!(
//!     try_match!("123", (nb = ('0'..'9')+ => nb.parse().unwrap())),
//!     Some((123u8,)),
//! );
//! ```
//!
//! ## capture kinds
//! capture can be of different kinds depending on the properties of its expression.
//!
//! ### slice captures
//! the most common captures where the expression doesnt contain any nested capture.
//!
//! they just extract the matched section through [`MatchAble::slice`], and have a default type of [`MatchAble::Slice`].
//!
//! they support [mapping](#mapping), also they apply [`Into::into`] on the matched section if a [type](#type-specifier) is specified wihtout a [map](#mapping).
//!
//! ```
//! assert_eq!(try_match!("abc", (abc = 'a' 'b' 'c')), Some(("abc",)));
//! assert_eq!(
//!     try_match!("123", (nb = ('0'..'9')+ => nb.parse().unwrap())),
//!     Some((123u8,)),
//! );
//! assert_eq!(
//!     try_match!("abc", (abc: Cow<str> = 'a' 'b' 'c')),
//!     Some((Cow::Borrowed("abc"),)),
//! );
//! ```
//!
//! #### atomic captures
//! the atomic capture have expression of unmodified [value](#value-atoms), [range](#range-atom) and [call atoms](#call-atom).
//!
//! they resolve to the [`Capture`](Matcher::Capture) returned by the atom [`Matcher`].
//!
//! they support [mapping](#mapping) and [`Into::into`] convertion, and may need to specify their [type](#type-specifier) unless they are [call atom](#call-atom) to a local [term](gramex!#term).
//!
//! ```
//! assert_eq!(try_match!("abc", abc:{Box::new("abc")}), Some(("abc",)));
//! assert_eq!(
//!     try_match!("ababa", (aa = list<'a', 'b'> => aa[1..].to_vec())),
//!     Some((vec!["a", "a"],)),
//! );
//! ```  
//!
//! ### structural captures
//! structural captures have expressions containing nested captures.
//!
//! by default they resolve to tuple of the nested captures captured values, unless a [type](#type-specifier) is specified, then they construct that type as a struct of fields taken from the nested captures.
//!
//! if a [map expression](#mapping) is given, the nested captures values are binded as their names, and the capture type become the map result.
//!
//! ```
//! assert_eq!(try_match!("abc", a:'a' b:'b' c:'c'), Some(("a","b", "c")));
//! #[derive(PartialEq)]
//! struct ABC<'src> {
//!     a: &'src str,
//!     b: &'src str,
//!     c: &'src str,
//! }
//! assert_eq!(
//!     try_match!("abc", (abc: ABC = a:'a' b:'b' c:'c')),
//!     Some((ABC { a: "a", b: "b", c: "c" },)),
//! );
//! assert_eq!(
//!     try_match!("abc", (abc = a:'a' b:'b' c:'c' => [a, b, c])),
//!     Some((["a", "b", "c"],)),
//! );
//! ```
//!
//! ### enumerated captures
//! enumerated captures have root [or expression](#or-expression) containing nested captures.
//!
//! they resolve to a [specified](#type-specifier) enum where each or branch resolve to a specific variant.
//!
//! variants are specified from a single nested capture in the branch where they become `CapName(cap_type)`, a `None` variant is used if the branch doesnt have a capture.  
//!
//! ```
//! #[derive(PartialEq)]
//! enum Cap<'src> {
//!     None,
//!     A(&'src str),
//!     B(&'src str),
//! }
//! assert_eq!(
//!     try_match!("a", (cap: Cap = a:'a' | 'b' b:_| 'c')),
//!     Some((Cap::A("a"),)),
//! );
//! assert_eq!(
//!     try_match!("c", (cap: Cap = a:'a' | 'b' b:_| 'c')),
//!     Some((Cap::None,)),
//! );
//! ```
//!
//! ## generated items
//! in grammar decleration, [structural](#structural-captures) and [enumerated](#enumerated-captures) captures can generate there own items, structs and enums respectifilly.
//!
//! these types are generated by specifing `struct ident?` or `enum ident?` in the [type specifier](#type-specifier), these types are named as `ident` if given, else the pascal case of the capture name.
//!
//! the generated items are public, has a `'src` lifetime, derive `Debug`, and are defined directly in the outer scope.
//!
//! a [slice capture](#slice-captures) can generate a struct type, which become `Struct<'src>(MatchAble::Slice)`.
//!
//! ```
//! gramex!{
//!     for str;
//!     let cap_struct: struct = a:'a' (b: String = 'b') c:'c'..'z';
//!     let cap_enum: enum Enum = a:'a' | (b: struct = 'b' _[2]) | 'c';
//! }
//! // generate
//! struct CapStruct<'src> {
//!  a: &'src str,
//!     b: String,
//!     c: &'src str,
//! }
//! enum Enum<'src> {
//!     None,
//!     A(&'src str),
//!     B(B),
//! }
//! struct B<'src>(&'src str)
//! ```
use crate::*;
#[cfg(feature = "macros")]
use gramex_macro::*;
use std::ops::RangeInclusive;
