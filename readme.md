# gramex
grammar expressions, a common language for advance parsers.

gramex is a framework for building ergonomic, efficient and advance parsers, tokenizers and any form of grammar based transformers.

it simplifies parsing by providing simple matching constructs with expressive DSL, while also featuring enriched imperative experience for advance use cases.

# features
## powerful core
gramex is universal in its core, everything can be `MatchAble`, from `str` to `[u8]` to `YourTokenList`, provided the required infrastructure.

it also adhere to rust zero cost abstraction principle, it leverage the power of GATs to enable its `Matcher`s get monomorphized into highly optimized code doing only the required features.

it also utilize zero copy parsing, it uses and produces slices of the input by default to minimize allocations.

## simple DSL for simple cases
gramex feature its own custom DSL, inspired by the typical metasyntax language, it has rich semantics. including and not limited to: repetitions, negations, lookaheads, intersections, alternations and implications.

this grammar expressions support powerfull capturing abilities, with nesting and enumeration support, and mapping into auto generated types.

this expressions can be used everywhere, declared inside standalone definitions, or used inline in the normal code, and even enriching the imperative cursors.

## imperative cursors for advance cases
gramex doesnt only generate simple parsers, it can empowers advance parsers through its imperative mode: the parsing `cursor`s.

in the cursor mode, the parsing is done using unconstrained typical imperative flow, enriched and integrated with the whole gramex ecosystem, through declarative and composable utilities.

cursors are typically used with custom tokens lists, which can be automaticly derived with their own dedicated matchers.

# quick showcase
```
fn basics() {
    // `matches` return `true` if a value matches a pattern
    // patterns are separated by whitespace, and can be literals, paths and blocks
    let c = "c";
    assert!(matches!("abc", 'a' "b" c));

    // `~` matches without advancing, `!` matches one token if its pattern fails
    assert!(matches!("ac", ~"ac" !"b" 'c'));

    // '?': optional, '+': 1..inf repetition, `[n]`: exact repetition
    assert!(matches!("bbccc", "a"? 'b'+ 'c'[3]));

    // `..`: range matching, `_`: matches one token
    assert!(matches!("abc", 'a'..'z' _ ));

    // `|`: match any pattern, `&`: match all patterns
    assert!(matches!("a", 'a' | 'b' | 'c'));
    assert!(matches!("b", 'a'..'z' & !'c'));

    // captures: extract the matched section
    assert!(parse!("abc", 'a' bc:"bc").is_ok_and(|(bc,)| bc == "bc"));
}

// grammar declaration, define multiple matchers
grammar! {
    for str;
    let ident: String = ('a'..'z' | 'A'..'Z' | '0'..'9' | '_')+;
    // `=> expr` mapping of the matched section (binded as `nb`)
    let nb: i64 = '-'? ('0'..'9')+ => nb.parse().unwrap();
    // `path<args>` compound matchers, `list<item, sep>`: list of `sep` seprated `item`s
    let arr: Vec<Val<'src>> = '[' list:list<val, ','> ']' => list;
    // generate an enum `Val`, can also use predefined matchers
    let val: enum = true:"true" | false:"false" | ident:ident | nb:nb | arr:arr;
}

// (input, offset) tuple with some utilities, for ergonomic advance cases
type Cur<'src> = SimpleCursor<'src, str>;
fn parse_primary(cur: &mut Cur) -> Result<Expr, MatchError> {
    // `try_eat`: optional match by matcher, even from grammer declarations
    if let Some(num) = cur.try_eat(nb) {
        Ok(Expr::Num(num))
    } else if let Some(_ident) = cur.try_eat(ident) {
        Ok(Expr::Ident(_ident))
    } else {
        // become `expected one of identifier, number`
        cur.expected(["identifier", "number"])
    }
}
fn parse_expr(cur: &mut Cur) -> Result<Expr, MatchError> {
    // `eat`: match by a grammer expression
    let (name,) = eat!(cur, name:ident '=')?;
    let mut expr = parse_primary(cur)?;
    'op_loop: loop {
        // `match_map`: gramex `match` expression
        match_map!(cur, {
            '+' => expr = Expr::Add(Box::new(expr), Box::new(parse_primary(cur)?)),
            '-' => expr = Expr::Sub(Box::new(expr), Box::new(parse_primary(cur)?)),
            else => break 'op_loop,
        })
    }
    cur.eat(';')?;
    Ok(Expr::Let(name, Box::new(expr)))
}
```