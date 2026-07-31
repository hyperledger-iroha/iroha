# UDT Parser Application

This example will demonstrate the use of UDTs (User-Defined Terminals).
We assume that [apg-lite](https://www.npmjs.com/package/apg-lite), [apg-js](https://www.npmjs.com/package/apg-js), version 4.3.0 or higher,
and [terser](https://www.npmjs.com/package/terser) are devDependencies for the project.

### The Setup

We assume the file structure is similar to that in this repository. e.g.

```
|my-project
|--|udt-app/
|--|--|grammar.txt
|--|--|node-app.js
|--|--|node-grammar.js
|--|--|web-grammar.js
|--|--|web-min.js
|--|--|web-app.js
|--|--|web.html
|--|lib/
|--|--|parser.js
|--|--|web-parser.js
```

These are the files that you write.

- grammar.txt - the SABNF grammar that defines the parser
- node-app.js - the Node.js parser application
- web-app.js - the web page parser application
- web.html - the web page that will display the web application.

See the `basic-app/` for details about generating the remaining files.
(Replacing `basic-app/` with `udt-app/`)

### The Example Grammar

For our example we will parse a telephone number into its basic components.
The grammar comes from the [North American Numbering Plan](https://en.wikipedia.org/wiki/North_American_Numbering_Plan).
The "office code" has some moderately complex conditions in that the first digit may not be 0 or 1 and the following
two digits may not be 11. That is, 501 and 510 are OK but 511 is not.
Therefore, we will use a UDT to match that phrase.
The ABNF grammar we will use is:

```
phone-number = "(" area-code ")" u_office "-" subscriber
area-code = gt-2 not-9 digit
subscriber = 4digit
gt-2 = %d50-57
not-9 = %d48-56
digit = %d48-57
```

Our application will simply parse a telephone number of the form "(xxx)xxx-xxxx" and report
its `area code`, `office code` and `subscriber number` if the number is valid.

### The Node.js App

For the Node.js demonstration simply run:

```
node udt-app/node-app.js
```

### The Web Page App

For the web page demonstration, displaying `udt-app/web.html` in a browser should present the parser and allow the user
to parse any telephone number from the "number to parse" text area.

You can also switch the comments in `web.html` as such for the minified version:

```
<!--
<script src="../lib/web-parser.js" charset="utf-8"></script>
<script src="./web-grammar.js" charset="utf-8"></script>
<script src="./web-app.js" charset="utf-8"></script>
-->
<script src="./web-min.js" charset="utf-8"></script>
```
