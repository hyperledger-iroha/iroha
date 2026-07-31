# Basic Parser Application

This example will demonstrate the construction of a simple, basic parser both as a Node.js application and a web page application.
We assume that [apg-lite](https://www.npmjs.com/package/apg-lite), [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
and [terser](https://www.npmjs.com/package/terser) are devDependencies for the project.

### The Setup

We assume the initial file structure to be similar to that of this repository.

```
|my-project
|--|basic-app/
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
node node_modules/apg-js/bin/apg.sh --lite -i basic-app/grammar.txt -o basic-app/node-grammar
node node_modules/apg-js/bin/apg.sh --lite -n Grammar -i basic-app/grammar.txt -o basic-app/web-grammar
cat lib/web-parser.js basic-app/web-grammar.js basic-app/web-app.js > basic-app/temp
node node_modules/.bin/terser basic-app/temp -c -m -o basic-app/web-min.js
rm basic-app/temp
```

Let's take them one at a time.

- mkdir lib
  - create a `lib/` directory
- cp node_modules/apg-lite/lib/parser.js ./lib
  - get a copy of the `apg-lite` Node.js, ESM parser
- cp node_modules/apg-lite/lib/web-parser.js ./lib
  - get a copy of the `apg-lite` web parser
    (note that this only differes from `parser.js` in that the leading `export` statement has been commented out.)
- node node_modules/apg-js/bin/apg.sh --lite -i basic-app/grammar.txt -o basic-app/node-grammar
  - use `apg-js` with the `--lite` option to generate a Node.js grammar object from the SABNF `grammar.txt`
- node node_modules/apg-js/bin/apg.sh --lite -n Grammar -i basic-app/grammar.txt -o basic-app/web-grammar
  - use `apg-js` with the `--lite` option to generate a web grammar object from the SABNF `grammar.txt`
- cat lib/web-parser.js basic-app/web-grammar.js basic-app/web-app.js > basic-app/temp
  - concatenate the `web-parser.js`, `web-grammar.js` and `web-app.js` into a temporary single file
- node node_modules/.bin/terser basic-app/temp -c -m -o basic-app/web-min.js
  - use `terser` to minify the temporary, concatenated file
- rm basic-app/temp
  - delete the temporary file

Therefore, the final file structure will look like:

```
|my-project
|--|basic-app/
|--|--|grammar.txt
|--|--|node-app.js
|--|--|web-app.js
|--|--|web.html
|--|--|web-grammar.js
|--|--|node-grammar.js
|--|--|web-min.js
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

For the Node.js demonstration simply run:

```
node basic-app/node-app.js
```

### The Web Page App

For the web page demonstration, displaying `basic-app/web.html` in a browser should present the parser and allow the user
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
