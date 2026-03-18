# gramex
grammer expressions, a common language for advance parsers.

gramex is a library and a simple language for building parsers, tokenizers and other forms of grammer based transformers.

it simplify parsing by transforming a simple yet expressive grammer declerations into efficient reusable matcher functions.

# features
- **type agnostic matching:** parse `str`, byte slices `[u8]`, or custom token streams.
- **zero cost abstractions:** grammers compile down to highly optimized, near metal matcher functions.
- **rich grammar syntax:** native support for `repetitions`, alternations `|`, intersections `&`, ranges `..`, lookahead peeks `~`, and negations `!`.
- **powerful capturing & mapping:** extract sections, nested or enumerated, and map them into custom types.
- **extensible throught code**: just drop your custom matcher inside `{}` block.
- term based grammer defenition thought `gramex`, or inlined expression matching through `matches`] and `try_match`
- **batteries included**: comes with various built-in helpers and standard patterns.

# quick guide
```rust
// quick matching can be done using `matches` macro
// matches agianst items by literals, path or blocks
assert!(matches!("abc": str, "abc"));
let pat = "abc";
assert!(matches!("abc": str, pat));
assert!(matches!("bc": str, { &pat[1..] }));

// patterns are separated by whitespace
assert!(matches!("abc": str, 'a' 'b' 'c'));

// `?`: optional, `*`: +0 repetition, `+`: +1 repetition
// `[count]`: exact repetition, `[min..max]`: ranged repetition
assert!(matches!("abbccc": str, 'a'? 'b'+ 'c'[3]));

// `!`: matches one item if pattern doesnt match
// `~`: matches a pattern without advancing
assert!(matches!("cba": str, !'a' ~'b' "ba"));

// `_`: matches any, `..` range match
assert!(matches!("abc": str, 'a'..'z' _ 'c'));

// `|`: match any of the pattern
// `&`: match if all patterns matches
assert!(matches!("b": str, 'a' | 'b' | 'c'));
assert!(matches!("b": str, 'a'..'z' & !'c'));

// `cond -> expr` matches `expr` if `cond` matches
assert!(matches!("ad": str, 'a' ('b' -> "bc") 'd'));

// capture are done using `(name = pattern)`
assert!(try_match!("abc": str, 'a' (bc = "bc")).is_ok_and(|v| v.bc == "bc"));
```