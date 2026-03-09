//! # grammer reference
//! ```txt
//! "abc" '-'? hex+ | _[3] & !"abc" & 'a'..'z' | (str: String = '"' !'"'* '"') !~','   
//! ```
//! gramex grammer syntax is inspired by the standared metasyntax languages (notably **Wirth syntax notation (WSN)**) and regular expressions, with a rust flavour.
//!
//! gramex grammer works on stream of tokens, this stream implements the [`MatchAble`] trait.
//!
//! gramex is composed of expressions that define patterns to match agianst that stream.
//!
//! gramex has the following expressions:
//! - **unit**: a matcher with modifiers.
//! - **range**: match a token against a range.
//! - **sequence**: matches by a sequence of expressions.
//! - **or**: match by one of given expressions.
//! - **and**: match by a list of expressions against the same input.
//! - **capture**: match a section then capture it.
//!
//! precedence: `capture` > `range` > `unit` > `and` > `sequence` > `or`.
//!
//! # atoms
//! ```
//! 'a' 1 "abc" _
//! ident path::to::matcher list<alpha+, ','>
//! { value.field } { |value, ind, status| ... } ( 'a' 'b' 'c' )
//! ```
//! atoms are the fundimental unit of the grammer, they are who do the actual token to token matching.
//!
//! atoms primary resolve into values that the stream implements [`MatchBy`] for.
//!
//! atoms have the following types:
//! - **literal**: any rust literal: strings, numbers, booleans, floats that the stream implement [`MatchBy`] for.
//! ```
//! assert!(matches!("abc": str, 'a' "bc"));
//! assert!(!matches!("cba": str, "abc"));
//! ```
//! - **path**: a path to an matching item (constant / static / fn / unit struct).
//! ```
//! mod example {
//! 	pub const pat: &str = "b";
//! }
//! static pat: &str = "a";
//! static pat2: &str = "c";
//! assert!(matches!("abc": str, pat example::pat self::pat2));
//! ```
//! - **`_`**: skips a single token, calls [`MatchAble::skip_1`].
//! - **block**: a block expression that resolve to a matching value, it is called on each iteration.
//! ```
//! let pats = ['a', 'b'];
//! assert!(matches!("abc": str, { pats[0] } { pat[1] } { a(|v| v == 'c') } ));
//! ```
//! - **group**: an expression wrapped inside parenthesis.
//! ```
//! assert!(matches!("ab123": str, ('a'..'z')+ ('0'..'9' | '-')*));
//! ```
//! - **call**: match by a parenthesized matcher called with a set of matchers created from the passed expressions.
//! ```
//! assert!(matches!("a,b,c": str, list<alpha+, ','>));
//! ```
//!
//! # unit
//! ```
//! "abc" !'a' ~dec hex? !('a'..'z')[3..5]
//! ```
//! an atom with modifiers.
//!
//! ### modifiers     
//! the modifiers changes the behavior of the matched atom, the are prefixed to the target atom.
//!
//! - **not (`!`)**: match one token if the atom doesn't match.    
//! it only matches one token even if the atom last for multiple ones.     
//! it fails on incomplete input.
//! ```
//! assert!(matches!("b": str, !'a'));
//! assert!(matches!("b": str, !"abc"));
//! assert!(!matches!("abc": str, !"abc"));
//! ```
//! - **near (`~`)**: matches an atom without advancing.     
//! can be prefixed with the not modifier to invert the result of the atom (doesnt fail on incomplete input).
//! ```
//! assert(matches!("abc": str, ~'a' ~"abc" !~dec _[3]));
//! ```
//!
//! ### repetition      
//! repetition specifies how many times an atom is matched.
//!
//! - **optional (`?`)**: match the atom 0 or 1 times.
//! ```
//! assert(matches!("ab": str, 'a'? 'b' 'c'?));
//! ```
//! - **multi (`*`)**: match the atom 0 or more times.
//! ```
//! assert(matches!("aaac": str, 'a'* 'b'* 'c'*));
//! ```
//! - **plus! (`+`)**: match the atom 1 or more times.
//! ```
//! assert(matches!("aaab": str, 'a'+ 'b'+));
//! assert(!matches!("aaac": str, 'a'+ 'b'+ 'c'+));
//! ```
//! - **exact (`[count]`)**: match the atom exactly `count` times.
//! ```
//! assert(matches!("aaabb": str, 'a'[3] 'b'[2]));
//! assert(!matches!("a": str, 'a'[2]));
//! assert(!matches!("aaa": str, 'a'[2]));
//! ```
//! - **range (`[min..max]`)**: match the atom between `min` and `max` (inclusive) times.           
//! `min` and `max` are optional, they defaults to `0` and `inf` respectively.     
//! ```
//! assert(matches!("aaabbcccc": str, 'a'[2..4] 'b'[..3] 'c'[3..]));
//! assert(!matches!("a": str, 'a'[2..]));
//! assert(!matches!("aaaaa": str, 'a'[2..4]));
//! ```
//!
//! `?` is `[0..1]`, `*` is `[0..]`, `+` is `[1..]`, none is `[1]`.
//!
//! unended repetition is greedy stoping only at mismatch or end of input, you can use `!"end"* & "pat"[..]` to control it.
//! ```
//! // locate the end `abc` then slice it and run `"ab"*` on it
//! assert(matches!("ababababc": str, !'abc'* & "ab"* _[3]));
//! ```
//!
//! in `?` modifier, the repetition take precedence, in `~` modifier the `~` take precedence.
//! ```
//! assert(!matches!("bbc": str, !~'b'[3] ~'b'[2] !'a'[3]));
//! ```
//!
//! # range
//! range expression (`lh`..`rh`) match a single token and check if it is between `lh` and `rh` inclusive.
//!
//! `lh` and `rh` must be only literal, path, block atoms without any modifiers or repetition, or a group of that kind of atoms.
//!
//! `lh` and `rh` can be of different atom types but must resolves to the same type.
//!
//! range expressions resolves to [`RangeInclusive`].
//! ```
//! assert(matches!("abc": str, 'a'..'z' !('0'..'9') ('a'..'z')+));
//! assert(!matches!("1": str, 'a'..'z'));
//! ```
//!
//! # sequence
//! sequence expression is a list of expressions separated by whitespace and matched in order.
//! ```
//! assert(matches!("abc": str, 'a' 'b' 'c'));
//! ```
//!
//! # or
//! or expression is a `|` separated list of expressions that matches one of them.
//!
//! the first expression that matches wins, the rest are ignored, if none matches the error is that of last.
//!
//! or expression has the lowest precedence, so it wrap all expressions till the `|` or the end.
//! ```
//! assert(matches!("a": str, 'a' | 'b' 'c' | "abc"));
//! assert(matches!("bcd": str, ('a' | 'b' 'c' | "abc") 'd'));
//! assert(!matches!("d": str, 'a' | 'b' 'c' | "abc"));
//! // 'a' wins, "bc" left
//! assert(!matches!("abc": str, 'a' | 'b' 'c' | "abc"));
//! ```
//!
//! # and
//! and expression is a `&` separated list of expressions that matches all of them against the same input.
//!
//! the first expression specifies the matched section, then the rest matches agianst that section where excess input is ignored. the first fail win.
//!
//! and expression has higher precedence that sequence.
//!
//! ```
//! assert(matches!("abc": str, ('a'..'z')[3] & ('a' _ _));
//! assert(matches!("abc": str, ('a'..'z')[3] & 'a' & (_ 'b'));
//! assert(!matches!("abc": str, ('a'..'z')[3] & !'abc' & { touch(|_| print!("not reached") });
//! ```
//!
//!
use crate::*;
use std::ops::RangeInclusive;
