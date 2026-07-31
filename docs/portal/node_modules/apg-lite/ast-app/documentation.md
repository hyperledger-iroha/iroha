# Abstract Syntax Tree (AST) Application

This example will demonstrate the construction of the AST, when and why it is useful and how to use it.
We assume that [apg-lite](https://www.npmjs.com/package/apg-lite) and [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
are devDependencies for the project.

### The Setup

We assume the initial file structure to be similar to that of this repository.

```
|my-project
|--|ast-app/
|--|--|grammar.txt
|--|--|parser-app.js
|--|--|ast-app.js
|--|--|node-grammar.js
|--|lib/
|--|--|parser.js
```

The files that you write are:

- grammar.txt - the SABNF grammar that defines the parser
- parser-app.js - the parser callbacks application
- ast-app.js - the AST callbacks application

The remaining files can be generated with:

```
mkdir lib
cp node_modules/apg-lite/lib/parser.js ./lib
node node_modules/apg-js/bin/apg.sh --lite -i basic-app/grammar.txt -o basic-app/node-grammar
```

That is,

- mkdir lib
  - create a `lib/` directory
- cp node_modules/apg-lite/lib/parser.js ./lib
  - get a copy of the `apg-lite` Node.js, ESM parser
- node node_modules/apg-js/bin/apg.sh --lite -i ast-app/grammar.txt -o ast-app/node-grammar
  - use `apg-js` with the `--lite` option to generate a Node.js grammar object from the SABNF `grammar.txt`

### The Example Grammar

For this example we will use a modified version of the "authority" from the URI RFC 3986 grammar.

```
;
; authority from the URI RFC 3986
; NOTE: While we have greatly simplified "userinfo" and "host",
;       the simplifications retain the fact that there is a great
;       deal of overlap in the characters space accepted by the two phrases.
;       That is, "userinfo" and "host" could easily both accept the same phrase.
;       And that is the essential problem this grammar is designed to emphasize.
;
authority     = [ userinfo "@" ] host [ ":" port ]
host          = reg-name
;userinfo      = *( unreserved / pct-encoded / sub-delims / ":" )
;host          = IP-literal / IPv4address / reg-name
;reg-name      = *( unreserved / pct-encoded / sub-delims )
userinfo      = *( ALPHA / DIGIT / "." / ":" )
reg-name      = *( ALPHA / DIGIT/ "." )
port          = *DIGIT
ALPHA         = %d65-90 / %d97-122
DIGIT         = %d48-57
```

We will first attempt to parse it with parser callback functions and see the problem that presents.
Then we will translate the "authority" with AST callback functions and see that it solves the problem
presented in our first attempt.
Finally, we will demonstrate conversion of the AST to XML.

### The Parser App

For the parser callback function demonstration simply run:

```
node ast-app/parser-app.js
```

### The AST App

For the AST callback function demonstration simply run:

```
node ast-app/ast-app.js
```
