# APG Lite

`apg-lite` is a light-weight parser of [ABNF](https://www.rfc-editor.org/rfc/rfc5234) grammars.
It is a parser only.
It relies on [apg-js](https://github.com/ldthomas/apg-js), version 4.3.0 or higher, with the new `--lite` option
to generate ESM grammar objects.
`apg-lite` features:

- Parses only JavaScript strings, whereas `apg-js` can parse any arbitrary array of positive integers.
- Retains only three of the eight [SABNF](https://sabnf.com/docs/python/md_docs_SABNF.html) superset operators of `apg-js`.
  - UDT, (User-Defined Terminals) Handwritten, phrase-matching code snippets.
  - AND, The positive look-ahead operator (see [syntactic predicates](https://en.wikipedia.org/wiki/Syntactic_predicate)).
  - NOT, The negative look-ahead operator.
- The AST has been simplified (see `./ast-app/`).
- Tracing (debugging) the parse tree has been simplified (see `./trace-app/`).
- Statistics collection (profiling) has been simplified (see `./trace-app/`).
- `apg-lite` is fully contained in a single JavaScript file.
  - `./lib/parser.js` contains ECMAScript Modules (ESM) which can be `import`-ed into your application.
  - `./lib/web-parser.js` can be scripted into your web page application with no bundling necessary.

A number of example application are included here to demonstrate the use of `apg-lite` in both
Node.js and web page applications. Each example application is in its own directory.
The documentation for each is in the file `documentation.md`.
Most (but not all) examples will be presented as both Node.js and web page applications.

- `./basic-app/` - A simple parser demonstrating the most basic construction and use of a parser.
- `./ast-app/` - Use of the AST - how to construct the AST and translate or semantically manipulate the phrases captured by the AST nodes.
  Demonstrates generating an XML version of the AST.
- `./udt-app/` - Use of a UDT, a handwritten code snippet to recognize a somewhat complicated phrase.
- `./lookahead-app/` - Demonstration of the look ahead operators.
- `./trace-app/` - How to debug and profile the parse tree.
  - Tracing is the primary means of debugging a grammar or input string.
  - Counting node hits serves as a type of profiling, exposing which rules are most often hit.
- `./cool-app/` - A simple demonstration of parsing a string with UTF+32 (in this case emoticon) characters.
- `./uri-app/` - This example builds a light-weight, but robust and well-tested URI parser.

The Node.js applications can all be run from the `package.json` scripts.
Use `npm run` to see the script names of the demonstrations.

### The URI Parser

Note: version 1.0.5 fixes a bug in the URI grammar.
The previous versions incorrectly allowed empty strings for the IPv4 octets.
No changes to the `apg-lite` parser or any of the other examples have been made.

The `UriParser` object in the `./uri-app/` application is of special interest.
It is a fully-contained (no dependencies) [URI](https://www.rfc-editor.org/rfc/rfc3986) parser.
Any URI can be parsed with, for example:

> npm run uri -- http://user@example.com

If no argument is given, the example will parse a pre-chosen representive URI.

`UriParser` is well-tested with [Jest](https://jestjs.io/) unit tests in the `__tests__` directory.

> npm test

will run all of the unit tests.

See `./uri-app/documentation.md` for complete documentation.
See `./uri-app/node-app.js` for a sample Node.js application
and `./uri-app/web.html` for sample web page application.
