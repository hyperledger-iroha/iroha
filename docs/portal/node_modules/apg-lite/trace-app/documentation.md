# Demonstrate Parse Tree Tracing and Statistics

In this example we will use the same floating point number grammar as in the `basic-app/`.
However, we will demonstrate how to trace the parser and collect node hit statistics.

When a parse fails the problem may be with the SABNF grammar or with the input string being parsed.
Our debugger in these cases is a trace of the parse tree that lets us see exactly what SABNF operators
were attempting to match a phrase to what input string. A close examination will nearly always show
you what and where the parse went wrong.

Sometimes, especially for large grammars with many rules, it is instructive to see which rules are
getting hit most often. This may allow some optimization of the SABNF grammar in order to improve
the parser's performance.

In short, `Trace` is the parser's debugger and `Stats` is the parser's profiler.

We assume that [apg-lite](https://www.npmjs.com/package/apg-lite), [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
and [terser](https://www.npmjs.com/package/terser) are devDependencies for the project.

### The Setup

We assume the initial file structure to be similar to that of this repository.

```
|my-project
|--|trace-app/
|--|--|grammar.txt
|--|--|node-app.js
|--|--|web-app.js
|--|--|web.html
```

These are the files that you write.

- grammar.txt - the SABNF grammar that defines the parser
- node-app.js - the Node.js parser application
- web-app.js - the web page parser application
- web.html - the web page that will display the web application.

All remaining files necessary to complete the project can be generated from these and the devDependencies.
The full set of commands (executed from the project directory, `my-project/`) for this are:

```
mkdir lib
cp node_modules/apg-lite/lib/parser.js ./lib
cp node_modules/apg-lite/lib/web-parser.js ./lib
node node_modules/apg-js/bin/apg.sh --lite -i trace-app/grammar.txt -o trace-app/node-grammar
node node_modules/apg-js/bin/apg.sh --lite -n Grammar -i trace-app/grammar.txt -o trace-app/web-grammar
cat lib/web-parser.js trace-app/web-grammar.js trace-app/web-app.js > trace-app/temp
node node_modules/.bin/terser trace-app/temp -c -m -o trace-app/web-min.js
rm trace-app/temp
```

Let's take them one at a time.

- mkdir lib
  - create a `lib/` directory
- cp node_modules/apg-lite/lib/parser.js ./lib
  - get a copy of the `apg-lite` Node.js, ESM parser
- cp node_modules/apg-lite/lib/web-parser.js ./lib
  - get a copy of the `apg-lite` web parser
    (note that this only differes from `parser.js` in that the leading `export` statement has been commented out.)
- node node_modules/apg-js/bin/apg.sh --lite -i trace-app/grammar.txt -o trace-app/node-grammar
  - use `apg-js` with the `--lite` option to generate a Node.js grammar object from the SABNF `grammar.txt`
- node node_modules/apg-js/bin/apg.sh --lite -n Grammar -i trace-app/grammar.txt -o trace-app/web-grammar
  - use `apg-js` with the `--lite` option to generate a web grammar object from the SABNF `grammar.txt`
- cat lib/web-parser.js trace-app/web-grammar.js trace-app/web-app.js > trace-app/temp
  - concatenate the `web-parser.js`, `web-grammar.js` and `web-app.js` into a temporary single file
- node node_modules/.bin/terser trace-app/temp -c -m -o trace-app/web-min.js
  - use `terser` to minify the temporary, concatenated file
- rm trace-app/temp
  - delete the temporary file

Therefore, the final file structure will look like:

```
|my-project
|--|trace-app/
|--|--|grammar.txt
|--|--|node-app.js
|--|--|node-grammar.js
|--|--|web-app.js
|--|--|web-grammar.js
|--|--|web-min.js
|--|--|web.html
|--|lib/
|--|--|parser.js
|--|--|web-parser.js
```

At this point, we can run both the Node.js demonstration, `node-app.js`,
and web page demonstration, `web-app.js` using either the full web files or the minified version.

### The Example Grammar

For this example we will parse a floating point number into its basic components.
The ABNF grammar we will use is:

```
float    = [sign] decimal [exponent]
sign     = "+" / "-"
decimal  = integer [dot [fraction]]
           / dot fraction
integer  = 1*%d48-57
dot      = "."
fraction = 1*%d48-57
exponent = "e" [esign] exp
esign    = "+" / "-"
exp      = 1*%d48-57
```

Our application will simply parse a floating point number and report its `sign`, `decimal` and `exponent`
if the number is valid.

### The Node.js App

The Node.js demonstration will generate a lot of output to the console.
It is recommended to pipe it to a temporary file so that it may be examined
later at your leasure. e.g.

```
node trace-app/node-app.js > /tmp/trace
less /tmp/trace
```

### The Web Page App

For the web page demonstration, displaying `trace-app/web.html` in a browser should present the parser and allow the user
to parse any floating point number from the "number to parse" text area.

By switching the comments in the HTML code section below you can run either the full web application or its minified version.

```
<!--
<script src="../lib/web-parser.js" charset="utf-8"></script>
<script src="./web-grammar.js" charset="utf-8"></script>
<script src="./web-main.js" charset="utf-8"></script>
-->
<script src="./web-min.js" charset="utf-8"></script>
```
