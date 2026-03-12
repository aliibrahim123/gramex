//! # Grammar Reference
//! ```text
//! "abc" '-'? hex+ | _[3] & !"abc" & 'a'..'z' | (str: String = '"' !'"'* '"') !~','   
//! ```
//! The `gramex` grammar syntax is inspired by standard metasyntax languages (notably **Wirth syntax notation (WSN)**) and regular expressions, but with a native Rust flavor.
//!
//! The `gramex` grammar works on a stream of tokens; this stream must implement the [`MatchAble`](`crate::MatchAble`) trait.
//!
//! `gramex` is composed of expressions that define patterns to match against that stream.
//!
//! `gramex` features the following expressions:
//! - **[`unit`](#unit)**: A matcher with modifiers.
//! - **[`range`](#range)**: Matches a token against an inclusive range.
//! - **[`sequence`](#sequence)**: Matches a sequence of expressions.
//! - **[`or`](#or)**: Matches exactly one of the given expressions.
//! - **[`and`](#and)**: Matches multiple expressions against the exact same input.
//! - **[`capture`](#capture)**: Matches a section and captures its value.
//!
//! **Precedence:** `capture` > `range` > `unit` > `and` > `sequence` > `or`.
//!
//! # Atoms
//! ```text
//! 'a' 1 "abc" _
//! ident path::to::matcher list<alpha+, ','>
//! { value.field } { |value, ind, status| ... } ( 'a' 'b' 'c' )
//! ```
//! Atoms are the fundamental units of the grammar; they perform the actual token-to-token matching.
//!
//! Atoms primarily resolve into values for which the stream implements [`MatchBy`](crate::MatchBy).
//!
//! Atoms come in the following types:
//! #### Literal
//! Any [literal](https://doc.rust-lang.org/stable/reference/tokens.html#literals) (strings, numbers, booleans, floats) that the stream implements [`MatchBy`](crate::MatchBy) for.
//! ```rust
//! # use crate::{matches};
//! assert!(matches!("abc": str, 'a' "bc"));
//! assert!(!matches!("cba": str, "abc"));
//! ```
//! #### Path
//! A [path](https://doc.rust-lang.org/stable/reference/paths.html#simple-paths) to a matching item (constant, static, function, or unit struct).
//! ```rust
//! # use crate::*;
//! mod example {
//!     pub const PAT: &str = "b";
//! }
//! static PAT1: &str = "a";
//! static PAT2: &str = "c";
//! assert!(matches!("abc": str, PAT1 example::PAT PAT2));
//! ```
//! #### Skip (`_`)
//! Skips a single token. Under the hood, this calls [`MatchAble::skip_1`](crate::MatchAble::skip_1).
//! #### Block
//! A [block expression](https://doc.rust-lang.org/stable/reference/expressions/block-expr.html) that resolves to a matching value. It is evaluated on each iteration.
//! ```rust
//! # use crate::*;
//! let pats = ['a', 'b'];
//! assert!(matches!("abc": str, { pats[0] } { pats[1] } { a(|v| v == 'c') } ));
//! ```
//! #### Group
//! An expression wrapped inside parentheses.
//! ```rust
//! # use crate::*;
//! assert!(matches!("ab123": str, ('a'..'z')+ ('0'..'9' | '-')*));
//! ```
//! #### Call
//! Matches using a [parameterized matcher](glossary#parameterized-matcher), called with a set of matchers created from the passed expressions.
//! ```rust
//! # use crate::*;
//! assert!(matches!("a,b,c": str, list<alpha+, ','>));
//! ```
//!
//! # Unit
//! ```text
//! "abc" !'a' ~"dec" hex? !('a'..'z')[3..5]
//! ```
//! A unit is an [`atom`](#atom) combined with modifiers.
//!
//! ### Modifiers     
//! Modifiers change the behavior of the matched atom; they are prefixed to the target atom.
//!
//! #### Not (`!`)
//! Matches exactly one token if the atom doesn't match.
//!  
//!  It always consumes exactly one token upon success, even if the negated pattern spans multiple tokens.
//!      
//!  It will fail on incomplete input.
//! ```rust
//! # use crate::*;
//! assert!(matches!("b": str, !'a'));
//! assert!(matches!("b": str, !"abc"));
//! assert!(!matches!("abc": str, !"abc"));
//! ```
//! #### Near (`~`)
//! Matches an atom without advancing the stream.     
//!  
//! Can be prefixed with the `!` modifier to invert the result of the lookahead (which does not fail on incomplete input).
//! ```rust
//! # use crate::*;
//! assert!(matches!("abc": str, ~'a' ~"abc" !~"dec" _[3]));
//! ```
//!
//! ### Repetition      
//! Repetition specifies how many times an atom should be matched, suffixed to the atom.
//!
//! #### Optional (`?`)
//! Matches the atom 0 or 1 time.
//! ```rust
//! # use crate::*;
//! assert!(matches!("ab": str, 'a'? 'b' 'c'?));
//! ```
//! #### Multi (`*`)
//! Matches the atom 0 or more times.
//! ```rust
//! # use crate::*;
//! assert!(matches!("aaac": str, 'a'* 'b'* 'c'*));
//! ```
//! #### Plus (`+`)
//! Matches the atom 1 or more times.
//! ```rust
//! # use crate::*;
//! assert!(matches!("aaab": str, 'a'+ 'b'+));
//! assert!(!matches!("aaac": str, 'a'+ 'b'+ 'c'+));
//! ```
//! #### Exact (`[count]`)
//! Matches the atom exactly `count` times.
//! ```rust
//! # use crate::*;
//! assert!(matches!("aaabb": str, 'a'[3] 'b'[2]));
//! assert!(!matches!("a": str, 'a'[2]));
//! assert!(!matches!("aaa": str, 'a'[2]));
//! ```
//! #### Range (`[min..max]`)
//! Matches the atom between `min` and `max` (inclusive) times.
//!
//! `min` and `max` are optional; they default to `0` and infinity, respectively.     
//! ```rust
//! # use crate::*;
//! assert!(matches!("aaabbcccc": str, 'a'[2..4] 'b'[..3] 'c'[3..]));
//! assert!(!matches!("a": str, 'a'[2..]));
//! assert!(!matches!("aaaaa": str, 'a'[2..4]));
//! ```
//! <br>
//!
//! **Note**: `?` is `[0..1]`, `*` is `[0..]`, `+` is `[1..]`, and no repetition implies `[1]`.
//!
//! Unbounded repetition is greedy, stopping only at a mismatch or the end of input. You can use intersections (`&`) to control bounds.
//! ```rust
//! # use crate::*;
//! // locate the end `abc` then slice it and run `"ab"*` on it
//! assert!(matches!("ababababc": str, !"abc"* & "ab"* _[3]));
//! ```
//!
//! With the `!` modifier, repetition takes precedence. With the `~` modifier, the `~` takes precedence.
//! ```rust
//! # use crate::*;
//! assert!(!matches!("bbc": str, !~'b'[3] ~'b'[2] !'a'[3]));
//! assert!(!matches!("bbc": str, !~('b'[3]) ~('b'[2]) (!'a')[3]));
//! ```
//!
//! # Range
//! Range expressions (`lh`..`rh`) match a single token and check if it falls between `lh` and `rh` inclusively.
//!
//! `lh` and `rh` must only be literals, paths, or block atoms (without any modifiers or repetitions), or a group containing such atoms.
//!
//! `lh` and `rh` can be of different atom types but must resolve to the same underlying type.
//!
//! Range expressions resolve to [`RangeInclusive`].
//! ```rust
//! # use crate::*;
//! assert!(matches!("abc": str, 'a'..'z' !('0'..'9') (('a'..'z'))+));
//! assert!(!matches!("1": str, 'a'..'z'));
//! ```
//!
//! # Sequence
//! A sequence expression is a list of expressions separated by whitespace and matched in order.
//! ```rust
//! # use crate::*;
//! assert!(matches!("abc": str, 'a' 'b' 'c'));
//! ```
//!
//! # Or
//! An OR expression is a `|`-separated list of expressions that matches exactly one of them.
//!
//! The first expression that matches wins, and the rest are ignored. If none match, the error returned is from the last expression evaluated.
//!
//! OR expressions have the lowest precedence, meaning they wrap all expressions until the next `|` or the end of the group.
//! ```rust
//! # use crate::*;
//! assert!(matches!("a": str, 'a' | 'b' 'c' | "abc"));
//! assert!(matches!("bcd": str, ('a' | 'b' 'c' | "abc") 'd'));
//! assert!(!matches!("d": str, 'a' | 'b' 'c' | "abc"));
//! // 'a' wins, leaving "bc" unmatched, which fails the overall match
//! assert!(!matches!("abc": str, 'a' | 'b' 'c' | "abc"));
//! ```
//!
//! # And
//! An AND expression is an `&`-separated list of expressions that matches all of them against the exact same input.
//!
//! The first expression specifies the bounded matched section. The rest of the expressions then match against that specific section, ignoring any excess input. The first failure wins.
//!
//! AND expressions have higher precedence than sequences.
//! ```rust
//! # use crate::*;
//! assert!(matches!("abc": str, ('a'..'z')[3] & ('a' _ _)));
//! assert!(matches!("abc": str, ('a'..'z')[3] & 'a' & (_ 'b')));
//! assert!(!matches!("abc": str, ('a'..'z')[3] & !"abc" & { touch(|_| print!("not reached") }));
//! ```
//!
//! # Captures
//! ```text
//! (ident = 'a'..'z' | 'A'..'Z' | '0'..'9' | '_')
//! (value*: String = nb | str | ident)
//! ```
//! Captures are matched sections that are extracted for later use.
//!
//! Captures are defined inside parentheses with the syntax `(name = expr)`, where `name` is the identifier and `expr` is the matched expression.
//!
//! Captures can occur in any expression except inside modified/repeated units, and inside call atom arguments.
//! ```text
//! (allowed1 = 1) (allowed2 = 2) & (allowed3 = 3) | (allowed4 = 4) ((allowed5 = 5 (allowed6 = 6)))
//! !(not_allowed1 = 1) (not_allowed2= 2))? list<(not_allowed3 = 3), ','>
//! ```
//! Captures can be repeated by suffixing their name with a repetition operator.
//!
//! `?` resolves to [`Option<T>`] and others resolve to [`Vec<T>`], where `T` is the type of the capture.
//! ```rust
//! # use crate::*;
//! assert_eq!(try_match!("abcd": str, 'a' (bc? = "bc") 'd').unwrap().bc, Some("bc"));
//! assert_eq!(try_match!("ad": str, 'a' (bc? = "bc") 'd').unwrap().bc, None);
//! assert_eq!(try_match!("abcbcbcd": str, 'a' (bc[2..5] = "bc") 'd').unwrap().bc, vec!["bc", "bc", "bc"]);
//! ```
//!
//! ### Capture Types
//! #### Normal
//! The matched expression does not contain any nested captures.
//!
//! These captures inherit the type of the matched section.
//! ```rust
//! # use crate::*;
//! assert_eq!(try_match!("abcd": str, 'a' (bc = "bc") 'd').unwrap().bc, "bc");
//! ```
//!
//! #### Term
//! Matches a local unparameterized term and inherits the type of that term.      
//!
//! Their matched expression must be a lone, unmodified path atom referring to that term.
//! ```rust
//! # use crate::*;
//! gramex! {
//!     for str;
//!     let ident = ('a'..'z' | 'A'..'Z' | '0'..'9' | '_')+;
//!     let value = (ident = ident);
//! }
//! assert_eq!(match_value("abc").unwrap().ident, "abc");
//! ```
//!
//! #### Structured
//! Captures that contain nested captures inside them.      
//!
//! These nested captures can occur in any allowed place except inside an OR expression.
//!
//! Structured captures generate their own type, a struct containing their inner captures as fields, plus a `matched` field containing the raw matched slice.
//! ```rust
//! # use crate::*;
//! let capture = try_match!("abcd": str, 'a' (bc = (b = 'b') (c = 'c')) 'd').unwrap().bc;
//! assert_eq!(capture.matched, "bc");
//! assert_eq!(capture.b, "b");
//! assert_eq!(capture.c, "c");
//! ```
//!
//! #### Enumerated
//!  The matched expression is an `or` expression containing nested captures.    
//!
//! The inner captures do not need to be the ORed expression directly; they can be nested deeper inside it. However, no two captures can exist in the exact same branch.     
//!
//! Not all OR branches need to contain captures.    
//!
//! Enumerated captures generate their own enum type, representing the inner captures as variants, plus a default `None` variant if not all branches have captures.
//! ```rust
//! # use crate::*;
//! gramex! { for str; let example = 'a' (bc = "bc" | (b2c3 = "bbccc") | (b3c1 = "bbbc")) 'd'; }
//! use example_captures::root_types::bc as BC;
//! assert_eq!(match_example("abcd").unwrap().bc, BC::None);
//! assert_eq!(match_example("abbcccd").unwrap().bc, BC::b2c3("bbccc"));
//! assert_eq!(match_example("abbbcd").unwrap().bc, BC::b3c1("bbbc"));
//! ```
//!
//! ### Capture Mapping
//! Captures can specify their mapped type through `(name: Type = expr)`. `Type` can be any Rust type, but it must implement[`From<T>`] if a mapping block is not provided.
//! ```rust
//! # use crate::*;
//! assert_eq!(try_match!("abcd": str, 'a' (bc: String = "bc") 'd').unwrap().bc, "bc");
//! ```
//!
//! Captures can also be dynamically mapped using a block, defined as `(name = expr => { map_block })`. The `map_block` is a regular Rust block that resolves to a `Fn(T) -> U`, converting the capture's matched type into the desired mapped type.
//!
//! Map blocks can be used even if no explicit type is specified, transforming the data while retaining the original type.
//! ```rust
//! # use crate::*;
//! assert_eq!(try_match!("abcd": str, 'a' (bc = "bc" => { |v| &v[1..] }) 'd').unwrap().bc, "c");
//! assert_eq!(try_match!("abcd": str, 'a' (bc: String = "bc" => { |v| v.to_uppercase() }) 'd').unwrap().bc, "BC");
//! ```
//!
//! Type specifiers and map blocks control the capture's base type before it is passed to a repetition container (like `Vec`), not the inverse.
//!
//! For more info about capture types, see the [`gramex`] macro documentation.

use crate::*;
use std::ops::RangeInclusive;
