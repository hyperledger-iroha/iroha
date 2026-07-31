# Look Ahead Application

This example will demonstrate the usefulness of the look ahead operators.
The negative look ahead operator (!) looks ahead at the next phrase and if it is matched, fails and backtracks.
The positive look ahead operator (&) looks ahead at the next phrase and if it is matched succeeds,
then backtracks.
Look ahead operators were originally called "syntactic predicates" and a good discussion of them is
[here](https://en.wikipedia.org/wiki/Syntactic_predicate).

We assume that [apg-lite](https://www.npmjs.com/package/apg-lite) and [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
are devDependencies for the project.

### The Setup

We assume the file structure to be similar to that of this repository.

```
|my-project
|--|lookahead-app/
|--|--|positive-grammar.txt
|--|--|negative-grammar.txt
|--|--|positive-grammar.js
|--|--|negative-grammar.js
|--|--|positive-app.js
|--|--|negative-app.js
|--|lib/
|--|--|parser.js
```

The files that you write are:

- positive-grammar.txt - the SABNF grammar with positive look ahead
- negative-grammar.txt - the SABNF grammar with negative look ahead
- positive-app.js - the Node.js application demonstrating the positive look ahead operator
- negative-app.js - the Node.js application demonstrating the negative look ahead operator

The remaining files can be generated with:

```
mkdir lib
cp node_modules/apg-lite/lib/parser.js ./lib
node node_modules/apg-js/bin/apg.sh --lite -i lookahead-app/positive-grammar.txt -o lookahead-app/positive-grammar
node node_modules/apg-js/bin/apg.sh --lite -i lookahead-app/negative-grammar.txt -o lookahead-app/negative-grammar
```

That is,

- mkdir lib
  - create a `lib/` directory
- cp node_modules/apg-lite/lib/parser.js ./lib
  - get a copy of the `apg-lite` Node.js, ESM parser
- node node_modules/apg-js/bin/apg.sh --lite -i lookahead-app/positive-grammar.txt -o lookahead-app/positive-grammar
  - use `apg-js` with the `--lite` option to generate a Node.js grammar object from the SABNF `positive-grammar.txt`
- node node_modules/apg-js/bin/apg.sh --lite -i lookahead-app/negative-grammar.txt -o lookahead-app/negative-grammar
  - use `apg-js` with the `--lite` option to generate a Node.js grammar object from the SABNF `negative-grammar.txt`

### The Positive Look Ahead Grammar

For this example we will use an SABNF grammar for the non-context-free language

> L = {a<sup>n</sup>b<sup>n</sup>c<sup>n</sup> | a &ge; 1}

```
; It is known that the language the language L = {a^nb^nc^n | n >= 1} is not context free.
; However, with the use of the look ahead operators we can construct a grammar
; that accepts those phrases and only those phrases.
S = &(AB !b) *a BC !c
AB = a [AB] b
BC = b [BC] c
a = %s"a"
b = %s"b"
c = %s"c"
```

The demonstration can be run with

```
node lookahead-app/positive-app.js
```

### The Negative Look Ahead Grammar

For this example we will use the standard C-language comment of the form /\* any characters \*/.

```
; Note that the rule "any-char" accepts "*" and "/".
; Without the negative look ahead operator *any-char would over run
; the end of the string and never find the "end" rule.
C-lang = begin comment end
begin    = "/*"
comment  = *(!end any-char)
end      = "*/"
any-char = %d32-126
```

The demonstration can be run with

```
node lookahead-app/negative-app.js
```

```

```
