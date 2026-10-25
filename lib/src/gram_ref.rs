//! # grammar reference
//! grammar expressions are matching constructs: a DSL inspired by typical metasyntax languages, with a Rust flavor.
//!
//! these expressions work on all [`MatchAble`] types, and they are transformed into efficient, [raw `Matcher`](Matcher#impl)-style matching logic.
//!
//! they can be used in different contexts, either as standalone items or inlined with code, through the macros provided by gramex: [`grammar!`], [`matches!`], [`try_match!`], [`check!`], [`parse!`], [`matcher!`], [`eat!`], [`try_eat!`], [`test!`] and [`match_map!`].
//!
//! grammar expressions are composed of [atoms](#atoms), which represent atomic patterns, and expression chains, which represent compound patterns.
//!
//! ```gramex
//! let expr = unit | seq | and | or | imply | capture;
//! ```
//!
//! # atoms
//! ```gramex
//! let atom = any_atom | value_atom | range_atom | call_atom | group_atom;
//! ```
//! atoms are the most primitive building blocks of the grammar; they define atomic patterns that cannot be split further in the expression chain.
//!
//! ### any
//! ```gramex
//! let any_atom = '_';
//! ```
//! the any atom (`_`) matches exactly one [token](MatchAble::Token), and fails only when the input is incomplete.
//!
//! under the hood, it calls [`MatchAble::skip_n(n: 1)`](MatchAble::skip_n) on the matched type.
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
//! value atoms match against a [`Matcher`] resolved from the tokens that define them.
//!
//! these tokens can be literals, [simple paths](https://doc.rust-lang.org/reference/paths.html#simple-paths) (including identifiers), and expression blocks, which are inserted directly into the generated code.
//!
//! the [`Matcher`] is called with only the required [`Mode` features](Mode#features), and its error is propagated directly.
//!
//! ```rust
//! assert!(matches!("a", 'a'));
//! mod consts {
//!     pub const B: char = 'b';
//! }
//! assert!(matches!("b", consts::B));
//! assert!(matches!("c", { a(|v| v == 'c') }));
//! ```
//!
//! ### range atom
//! ```gramex
//! let range_atom = value_atom ".." value_atom;
//! ```
//! range atoms match against a range of values defined by two [value atoms](#value-atoms).
//!
//! the bounds can be any value atoms, but they must resolve to the same underlying type.
//!
//! a range atom is transformed into a [`RangeInclusive`], which must implement [`Matcher`].
//!
//! ```
//! assert!(matches!("a", 'a'..'z'));
//! const Z: &str = "Z";
//! assert!(!matches!("1",  "A"..Z));
//! ```
//!
//! ### call atom
//! ```gramex
//! let matcher = ("for" matched_type:ty)? expr ("=>" rust_expr)?;
//! let call_atom = path "<" args:list<matcher, ','> ">";
//! ```
//! the call atom matches a compound matcher parameterized by a set of arguments.
//!
//! a compound matcher is a `Fn(..Matcher) -> Matcher`; the passed arguments are transformed into anonymous [`Matcher`]s and handed to the function, and the returned [`Matcher`] is then matched against.
//!
//! each argument is a matcher declaration consisting of a root expression, an optional matched-type specifier (defaulting to the current type), and an optional [map](#mapping) applied to the root [capture](#captures).
//!
//! ```rust
//! assert!(matches!("ababa", list<"a", "b">));
//! ```
//!
//! ### group atom
//! ```gramex
//! let group_atom = '(' expr ')';
//! ```
//! the group expression is a grammar expression enclosed in parentheses.
//!
//! ```
//! assert!(matches!("1", !('a'..'z')));
//! ```
//!
//! # units
//! ```gramex
//! let unit = not?:'!' near?:'~' atom rep?;
//! ```
//! units are [atoms](#atoms) with optional modifiers on top.
//!
//! the [range atom](#range-atom) cannot have these modifiers.
//!
//! ### not operator
//! the not operator `!` matches exactly one [token](MatchAble::Token) if the [atom](#atoms) does not match.
//!
//! the not operator also fails on incomplete input.
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
//! it can be combined with not to invert the result; incomplete input counts as a positive result for the negated form.
//!
//! ```
//! assert!(matches!("a", ~'a' _));
//! assert!(matches!("a", !~'b' _));
//! assert!(matches!("", !~'b'));
//! ```
//!
//! ## repetitions
//! ```gramex
//! let rep = opt:'?' | multi0:'*' | multi1:'+' | (exact = '[' count:nb ']') |
//!  (range: '[' min?:nb ".." max?:nb ']');
//! ```
//!
//! a repetition matches an [atom](#atoms) a number of times within an interval.
//!
//! #### shorthands
//! `?` matches the [atom](#atoms) zero or one times.
//! ```
//! assert!(matches!("a", "a"?));
//! assert!(matches!("", "a"?));
//! ```
//!
//! `*` matches the [atom](#atoms) zero or more times.
//! ```
//! assert!(matches!("aa", "a"*));
//! assert!(matches!("", "a"*));
//! ```
//!
//! `+` matches the [atom](#atoms) one or more times.
//! ```
//! assert!(matches!("aa", "a"+));
//! assert!(!matches!("", "a"+));
//! ```
//!
//! `[n]` matches the [atom](#atoms) exactly `n` times.
//! ```
//! assert!(matches!("aa", "a"[2]));
//! assert!(!matches!("a", "a"[2]));
//! ```
//!
//! #### canonical form
//! `[min..max]` matches the [atom](#atoms) between `min` and `max` times, inclusive.
//!
//! `min` and `max` are optional and default to `0` and infinity respectively.
//! ```
//! assert!(matches!("aaa", "a"[2..4]));
//! assert!(!matches!("a", "a"[2..4]));
//! assert!(matches!("a", "a"[..4]));
//! assert!(matches!("aaa", "a"[2..]));
//! ```
//!
//! #### notes
//! unbounded repetition is greedy, consuming input until a mismatch or the end of the input, while a bounded repetition stops immediately upon reaching `max`.
//! ```
//! assert!(matches!("aaab", "a"+ "b"));
//! assert!(!matches!("aaaa", "a"+ "a"));
//! assert!(matches!("aaaa", "a"[3] "a"));
//! ```
//!
//! with the [not operator](#not-operator), the repetition applies to the negated atom, while with the [near operator](#near-operator) (and its negated form), the operator applies to the whole repetition.
//!
//! ```
//! matches!("abc", !"d"[3]) // <=> matches!("abc", (!"d")[3])
//! matches!("aaa", ~"a"[3] _*) // <=> matches!("aaa", ~("a"[3]) _*)
//! ```
//!
//! # flow expressions
//! flow expressions combine other expressions through a control flow.
//!
//! from highest to lowest, their precedence is: [`unit`](#units) / [`capture`](#captures) > [`and`](#and-expression) > [`seq`](#sequence-expression) > [`imply`](#imply-expression) > [`or`](#or-expression).
//!
//! ## sequence expression
//! ```gramex
//! let seq = expr+;
//! ```
//! the sequence expression matches multiple whitespace-separated expressions in order.
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
//! the expressions are tried in order from the same start offset, and the first match wins.
//!
//! if a branch is an [imply](#imply-expression) expression and its condition matches, the or expression propagates the result of its `then` expression instead of trying the remaining branches.
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
//! the and expression matches all of the expressions separated by `&` against the same section.
//!
//! it first matches its first expression, then [slices](MatchAble::slice) the input up to where that expression ended and matches the remaining expressions against that slice, in order, from the same start.
//!
//! the and expression requires [`MatchAble::Slice`] to be a [`MatchAble`] itself.
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
//! the imply expression expresses a logical implication between two expressions.
//!
//! if the condition matches, it then matches the `then` expression; otherwise it succeeds without consuming any input.
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
//! a capture matches an expression, then extracts the matched section as its result.
//!
//! there are two capture syntaxes: a shorthand with an [atom](#atoms), `name:atom`, and a full form with an expression, `(name = expr)`.
//!
//! ```
//! assert_eq!(try_match!("abc", 'a' bc:"bc"), Some(("bc",)));
//! assert_eq!(try_match!("abc", 'a' (bc = !('0'..'9') 'c')), Some(("bc",)));
//! ```
//!
//! captures are allowed in most cases, but they are forbidden in [negated units](#not-operator), in repeated units whose repetition is specified and non-optional, in arguments of non-captured [call atoms](#call-atom), and in non-capturing macros such as [`matches!`].
//!
//! ```
//! try_match!("", !(_ not_allowed1:_) (_ not_allowed2:_)* list(not_allowed3:_, ','));
//! matches!("", not_allowed4:_);
//! assert_eq!(try_match!("a", (_ & (~(~_ allowed:_) -> _) | _)?), Some((Some("a"),)));
//! ```
//!
//! ### repetition
//! captures support [repetition](#repetitions), which is either inferred from the optionality of the path ([optional](#repetitions) units, [or](#or-expression) and [imply](#imply-expression) expressions) or specified manually.
//!
//! repetition changes the capture's resolved type: it becomes [`Option<T>`] if the repetition is optional, and [`Vec<T>`] otherwise.
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
//! the type specifier declares the capture's base type (excluding the [repetition](#repetition)) when needed.
//!
//! captures do not always require a type; it is only needed in contexts where the default type does not work: the root capture of [terms](grammar!#term) and matchers, and nested captures of [generated items](#generated-items).
//!
//! inside the type specifier, a `'src` lifetime is available, tied to the lifetime of the [matched value](MatchAble).
//!
//! ```
//! assert_eq!(
//!     try_match!("abc", a:'a' (bc = "bc" => bc.to_string())),
//!     Some(("a", "bc")),
//! );
//! grammar! {
//!     for str;
//!     let nb: u8 = ('0'..'9')+ => nb.parse().unwrap();
//! }
//! ```
//!
//! ### mapping
//! a mapping is an optional Rust expression that transforms the matched section into a different value or type.
//!
//! it is specified through `=> expr` after the expression, and the matched section is bound to the capture's name.
//!
//! note that using a mapping may require a [type specifier](#type-specifier).
//!
//! ```
//! assert_eq!(
//!     try_match!("123", (nb = ('0'..'9')+ => nb.parse().unwrap())),
//!     Some((123u8,)),
//! );
//! ```
//!
//! ## capture kinds
//! captures can be of different kinds, depending on the properties of their expressions.
//!
//! ### slice captures
//! the most common kind of capture: its expression contains no nested captures.
//!
//! it simply extracts the matched section through [`MatchAble::slice`], and its default type is [`MatchAble::Slice`].
//!
//! slice captures support [mapping](#mapping); if a [type](#type-specifier) is specified without a [map](#mapping), they apply [`Into::into`] to the matched section.
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
//! an atomic capture's expression is a single unmodified [value](#value-atoms), [range](#range-atom) or [call atom](#call-atom).
//!
//! it resolves to the [`Capture`](Matcher::Capture) returned by the atom's [`Matcher`].
//!
//! atomic captures support [mapping](#mapping) and [`Into::into`] conversion, and may require a [type specifier](#type-specifier), unless they are a [call atom](#call-atom) to a local [term](grammar!#term).
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
//! structural captures have expressions that contain nested captures.
//!
//! by default, they resolve to a tuple of the nested captures' values; if a [type](#type-specifier) is specified, they instead construct that type as a struct whose fields come from the nested captures.
//!
//! if a [map expression](#mapping) is given, the nested captures' values are bound to their names, and the capture's type becomes the type of the map result.
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
//! enumerated captures have a root [or expression](#or-expression) that contains nested captures.
//!
//! they resolve to a [specified](#type-specifier) enum, in which each `or` branch maps to a variant.
//!
//! each variant is derived from the single nested capture in its branch and becomes `CapName(cap_type)`; a `None` variant is used for branches without a capture.
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
//! in grammar declarations, [structural](#structural-captures) and [enumerated](#enumerated-captures) captures can generate their own items: structs and enums respectively.
//!
//! these types are generated by specifying `struct ident?` or `enum ident?` in the [type specifier](#type-specifier); they are named `ident` if given, otherwise after the capture name in PascalCase.
//!
//! the generated items are public, have a `'src` lifetime, derive `Debug`, and are defined directly in the outer scope.
//!
//! a [slice capture](#slice-captures) can generate a struct type, which becomes `Struct<'src>(MatchAble::Slice)`.
//!
//! ```
//! grammar!{
//!     for str;
//!     let cap_struct: struct = a:'a' (b: String = 'b') c:'c'..'z';
//!     let cap_enum: enum Enum = a:'a' | (b: struct = 'b' _[2]) | 'c';
//! }
//! // generates
//! struct CapStruct<'src> {
//!     a: &'src str,
//!     b: String,
//!     c: &'src str,
//! }
//! enum Enum<'src> {
//!     None,
//!     A(&'src str),
//!     B(B),
//! }
//! struct B<'src>(&'src str);
//! ```
use crate::*;
#[cfg(feature = "macros")]
use gramex_macro::*;
use std::ops::RangeInclusive;
