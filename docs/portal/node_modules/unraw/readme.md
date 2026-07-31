# `unraw`

```ts
unraw("\\'\\t\\u{1f601}\\'"); // -> "'	😁'"
```

`unraw` is a small module that converts raw strings to parsed strings in the same
manner as the standard JavaScript escaping engine. In essence, it is the exact
opposite of
[`String.raw`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/raw).

## Use Case

Most of the time, you probably don't need this library unless you're working
directly with raw strings and you need a way to get them back to normal strings.
Maybe the most signicant use case is when building
[template literal tags](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals#Tagged_templates);
you can use the `.raw` property of the passed string array to access the raw
strings, but then you may want to still return normal strings after processing.

The module is also useful for parsing text files written with escape sequences,
although keep in mind that the JavaScript flavor of escape sequences may differ
from the flavor used in an input file.

## Getting Started

`unraw` is a UMD module, so it can be used in Node or on the web. Typings are
included for TypeScript as well.

### Usage in Node.JS

`unraw` is hosted on [npm](https://www.npmjs.com/unraw), so you can install
with:

```bash
npm i unraw
```

To use in code:

```js
import unraw from "unraw";

unraw("\\n");
```

If you want to access error messages:

```js
import {unraw, errorMessages, ErrorType} from "unraw";

unraw("\\n");
errorMessages.get(ErrorType.MalformedUnicode);
```

### Usage in the Browser

You can embed it (minified) on a webpage with
[RequireJS](https://requirejs.org/). The module is available on
[UNPKG](https://unpkg.com) at https://unpkg.com/unraw:

```html
<script src="https://cdnjs.cloudflare.com/ajax/libs/require.js/2.3.6/require.min.js"></script>
<script>
  require(["https://unpkg.com/unraw^1.2.3/dist/index.min.js"], function(unraw) {
    unraw.unraw("\\n");
    unraw.errorMessages.get(unraw.ErrorType.MalformedUnicode);
  });
</script>
```

_Note_: Importing via the 'bare' url (`https://unpkg.com/unraw`) is not
supported as it breaks references to other required files.

## Usage

Usage is simple - the library exports a default function, `unraw`. The first
argument to `unraw` is the string to parse, and the second is an optional flag
to allow or disallow octal escapes, which are deprecated (defaults to
`false`, so the default behavior is to throw an error when octal sequences
are encountered).

```js
unraw("\\t\\tThis is indented.");
// => "		This is indented."
```

The library attempts to mimic the behaviour of standard JavaScript strings as
closely as possible. This means that invalid escape sequences will throw
`SyntaxError`s and that every escape sequence that is valid in a normal string
should be valid when passed to `unraw`.

In some ways this is similar to the behavior of `JSON.parse`.

You can always expect the outcome of calling `unraw` on a raw string to be
exactly the same as if that string were not raw in the first place:

```js
`Invalid: \u23`                   // Throws a SyntaxError
unraw(String.raw`Invalid: \u23`)  // Throws a SyntaxError

`Valid: \u0041`                   // => `Valid: A`
unraw(String.raw`Valid: \u0041`)  // => `Valid: A`

`Valid: \A`                       // => `Valid: A`
unraw(String.raw`Valid: \A`)      // => `Valid: A`

`Valid: \\`                       // => `Valid: \`
unraw(String.raw`Valid: \\`)      // => `Valid: \`

`Valid: \x42`                     // => `Valid: B`
unraw(String.raw`Valid: \x42`)    // => `Valid: B`

`Octal: \102`                      // => Throws a SyntaxError
unraw(String.raw`Octal: \102`)     // => Throws a SyntaxError
unraw(String.raw`Octal: \102`, true) // => Octal: B
```

### Errors

If desired, you can access the possible error messages to help identify errors:

```ts
import {unraw, ErrorType, errorMessages} from "unraw";

try {
  unraw("\\u25");
} catch (err) {
  if (err.message === errorMessages.get(ErrorType.MalformedUnicode)) {
    console.error("String had an invalid Unicode escape sequence.");
  }
}
```

The full list of error message names available through the `ErrorType` enum
(exposed as a normal object in JavaScript).

## Contributing

Found a bug? Please,
[submit it here.](https://github.com/iansan5653/unraw/issues)

Pull requests are always welcome, although to increase your chances of your
contribution being accepted, opening an issue and linking to it is always a
good idea.

Pull requests will not be merged unless the Azure Pipelines build succeeds.
This means that all checks must pass and code must be free of lint errors. To
quickly confirm that it will, just run:

```bash
npm run check
```

This checks your formatting, tests, and for TypeScript compiler errors. If the
task doesn't fail, you should be good to go.

### Other Tasks

For your convenience, some other tasks are also provided in the `package.json`:

- `npm run build` - Compiles TypeScript code to JavaScript
- `npm run minify` - Generate minified JavaScript files from compiled files
- `npm run test` - Quickly run tests using TypeScript code without compiling
- `npm run testWithCoverage` - Run tests and generate coverage report
- `npm run lint` - Check code for linting errors
- `npm run check` - Check to ensure code will pass Pipelines checks (see above)
- `npm run format` - Format code using Prettier
