# A URI (RFC3986) Parser Application

With this application we will build a real-world, well-tested URI parser that will work equally well in Node.js
and web page applications. The application weighs in at 78K &ndash; 39K for the minified web page application.

We assume that [apg-lite](https://www.npmjs.com/package/apg-lite), [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
[jest](https://jestjs.io/) and [terser](https://www.npmjs.com/package/terser) are devDependencies for the project.

### The Setup

Our file structure is:

```
|apg-lite
|--|uri-app/
|--|--|documentation.md
|--|--|grammar.txt
|--|--|grammar.js
|--|--|node-app.js
|--|--|node-grammar.js
|--|--|node-uri-parser.js
|--|--|web-app.js
|--|--|web-uri-min-parser.js
|--|--|web-uri-parser.js
|--|--|web.html
|--|lib/
|--|--|parser.js
|--|--|web-parser.js
```

These are the files and some comments and/or documentation.

- `documentation.md` - This file.
- `grammar.txt` - The SABNF URI grammar.
  - The original RFC 3986 grammar has been altered in several significant ways.
    - Literal strings are replaced with numbers and ranges (`%d32` & `%d32-126`, etc.) when possible.
      These operators are more efficient than literal string operators.
    - The RFC 3986 IPv6address does not work because of APG's "first-success disambiguation" and "greedy" repetitions.
      IPv6address is redefined and validations moved to callback functions (semantic vs syntactic validation).
      The redefinition requires negative [look-ahead operators](https://en.wikipedia.org/wiki/Syntactic_predicate).
      That is SABNF instead of simple ABNF.
    - The RFC 3986 IPv4address also fails because of "first-success disambiguation".
      This could be fixed with rearrangement of the alternative terms. However, it would still not
      accept zero-padded (leading zeros) decimal octets.
      Therefore, IPv4address is also done with callback functions and semantic validation.
    - The negative look-ahead operator is also needed in the definition of `host` to
      prevent failure of a `reg-name` that begins with an IPv4 address.
- `grammar.js` - The the grammar object generated from `grammar.txt` using `apg-js` with the `--lite` and `--name=Grammar` options.
- `node-app.js` - A simple application that imports the URI parser object from `node-uri-parser.js`.
- `node-grammar.js` - The the grammar object generated from `grammar.txt` using `apg-js` with the `--lite` option only.
  Used only in the `__tests__/uri-char.tests.js` unit testing.
- `node-uri-parser.js` - A self-executing function that returns a URI parser object. This file is self-contained in that
  in addition to the URI parser object code it has copies of `lib/web-parser.js` and `uri-app/grammar.js` imbedded.
  No other file or dependency is required of an application using the URI parser.
- `web-app.js` - A simple web page application that will parse a URI from web page input and display the results.
- `web-uri-min-parser.js` - The minified version of `web-uri-parser.js` using `terser` with the `--compress` and `--mangle` options.
- `web-uri-parser.js` - This file is identical to `node-uri-parser.js` except that the `export` statement is commented out.
- `web.html` - The web page that will display the web application.
- `lib/parser` - The `apg-lite` Node.js ESM parser.
- `lib/web-parser` - The `apg-lite` web page parser - only differs from `lib/parser` in that the `export` statement is commented out.

### The API

The URI parser object has a simple API. Assuming `obj` as the URI parser object:

- result = obj.parse(uri[, doTrace]);

  - uri is the URI to parse as a JavaScript string
  - doTrace if present and "truthy" will cause the APG parser to generate a trace of the parse tree
  - result

    - if the URI is valid (e.g. [https://user:pass@example.com:123/one/two.three?q1=a1&q2=a2#body]()):

      > {  
      > &nbsp;&nbsp;&nbsp;&nbsp;uri: '[https://user:pass@example.com:123/one/two.three?q1=a1&q2=a2#body]()',  
      > &nbsp;&nbsp;&nbsp;&nbsp;scheme: 'https',  
      > &nbsp;&nbsp;&nbsp;&nbsp;userinfo: 'user:pass',  
      > &nbsp;&nbsp;&nbsp;&nbsp;host: 'example.com',  
      > &nbsp;&nbsp;&nbsp;&nbsp;port: 123,  
      > &nbsp;&nbsp;&nbsp;&nbsp;path: '/one/two.three',  
      > &nbsp;&nbsp;&nbsp;&nbsp;query: 'q1=a1&q2=a2',  
      > &nbsp;&nbsp;&nbsp;&nbsp;fragment: 'body',  
      > }

    - or `undefined` if the URI is invalid

- result = obj.apgParserResult();
  - returns the `apg-js` parser result. e.g. (from the example above)
    > {  
    > &nbsp;&nbsp;&nbsp;&nbsp;success: true, (only if stateName is MATCH and the entire string is matched)  
    > &nbsp;&nbsp;&nbsp;&nbsp;state: 101, (the final `apg-js` state id for the start rule)  
    > &nbsp;&nbsp;&nbsp;&nbsp;stateName: 'MATCH', (the human-readable state id)  
    > &nbsp;&nbsp;&nbsp;&nbsp;length: 64, (the number of characters in the input string - the URI)  
    > &nbsp;&nbsp;&nbsp;&nbsp;matched: 64, (the number of characters actually matched)  
    > &nbsp;&nbsp;&nbsp;&nbsp;maxMatched: 64, (gives a hint where the parser failed if less than length)  
    > &nbsp;&nbsp;&nbsp;&nbsp;maxTreeDepth: 15, (the maximum depth of the actual parse tree)  
    > &nbsp;&nbsp;&nbsp;&nbsp;nodeHits: 389, (the number of parse tree nodes traversed)  
    > }
- text = obj.displayTrace();
  - if using obj.parse(uri, true), `text` is a display of the trace through the parse tree
    - caveat: This can generate a lot of output. Best to save it to a temporary file
      and peruse it with `less` or `vi` or your favorite text editor.
    - good for debugging an invalid URI _if_ you are familiar with `apg-js` and `grammar.txt`

### The Node.js App

Our demonstration will simply parse a URI with all the required and optional parts,
report the `apg-js` result and the URI parser result.

```
node uri-app/node-app.js
```

### The Web Page App

For the web page demonstration, display `uri-app/web.html` in a browser.
This will allow the user
to parse any URI from the "URI to parse" text area by pressing the "parse" button.

By switching the comments in the HTML code section below you can run either the full web application or its minified version.

```
<!--
<script src="./web-uri-parser.js" charset="utf-8"></script>
-->
<script src="./web-uri-min-parser.js" charset="utf-8"></script>
<script src="./web-app.js" charset="utf-8"></script>

or

<script src="./web-uri-parser.js" charset="utf-8"></script>
<!--
<script src="./web-uri-min-parser.js" charset="utf-8"></script>
-->
<script src="./web-app.js" charset="utf-8"></script>

```

### Unit Testing

This URI parser has been quite thoroughly unit tested. These tests can be run with:

- npm test -- uri-chars
  - A complete set of tests that strings such as `reserved`, `unreserved`,
    `gen-delims` and `sub-delims` have been correctly replaced with TBS (`%d48`) and TRG (`%d48-57`) operators.
- npm test -- uri-js
  - The same set of unit tests used by [uri-js](https://www.npmjs.com/package/uri-js)
    (not including the IPv6 Zone Identifier tests.)
- npm test -- uri-ipv4-ipv6
  - A large set of tests covering nearly every combination of valid
    and many invalid IPv4 and IPv6 addresses.
