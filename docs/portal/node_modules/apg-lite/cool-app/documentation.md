# Parsing 32-bit Characters in JavaScript

`apg-lite` accepts only JavaScript strings as input.
This example will demonstrate how to parse JavaScript strings with character codes greater than xFFFF.
We assume that [apg-lite](https://www.npmjs.com/package/apg-lite), [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
and [terser](https://www.npmjs.com/package/terser) are devDependencies for the project.

### The Setup

We assume the file structure is similar to that in this repository. e.g.

```
|my-project
|--|cool-app/
|--|--|grammar.txt
|--|--|node-app.js
|--|--|web-app.js
|--|--|web.html
|--|--|node-grammar.js
|--|--|web-grammar.js
|--|--|web-min.js
|--|lib/
|--|--|parser.js
|--|--|web-parser.js
```

The app functions that you write are:

- grammar.txt - the SABNF grammar that defines the parser
- node-app.js - the Node.js parser application
- web-app.js - the web page parser application
- web.html - the web page that will display the web application.

See the `basic-app/` for details about generating the remaining files.
(Replacing `basic-app/` with `cool-app/`)

### The Example Grammar

Our example is a trivial grammar that defines two lines,
each with an arbitrary statement (that may or may not make sense) ending with an emoticon.
Because `apg-lite` only accepts JavaScript strings we will have to enter the emoticons
as surrogate pairs.
The ABNF grammar we will use is:

```
start      = statement1 LF statement2 [LF]
statement  = %d33-126 *%d32-126
statement1 = statement grin
statement2 = statement cool
grin       = %d128513 ; grinning face
cool       = %d128526 ; cool shades, sunglasses
LF         = %d10 / %d13.10 / %d13
```

The surrogate pair for "grin"(128513) is '\uD83D\uDE01'.
The surrogate pair for "cool"(128526) is '\uD83D\uDE0E'.

Our application will simply parse and display the two statements.

### The Node.js App

For the Node.js demonstration simply run:

```
node cool-app/node-app.js
```

### The Web Page App

For the web page demonstration, displaying `cool-app/web.html` in a browser should present the parser and allow the user
to parse any two statements from the "statements to parse" text area.

You can also switch the comments in `web.html` as such to use the minifies version of the web application:

```
<!--
<script src="../lib/web-parser.js" charset="utf-8"></script>
<script src="./web-grammar.js" charset="utf-8"></script>
<script src="./web-app.js" charset="utf-8"></script>
-->
<script src="./web-min.js" charset="utf-8"></script>
```

The web page should work exactly as before but now with all the JavaScript minified.
