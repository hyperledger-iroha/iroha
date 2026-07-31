# @swaggerexpert/json-pointer

[![npmversion](https://img.shields.io/npm/v/%40swaggerexpert%2Fjson-pointer?style=flat-square&label=npm%20package&color=%234DC81F&link=https%3A%2F%2Fwww.npmjs.com%2Fpackage%2F%40swaggerexpert%2Fjson-pointer)](https://www.npmjs.com/package/@swaggerexpert/json-pointer)
[![npm](https://img.shields.io/npm/dm/@swaggerexpert/json-pointer)](https://www.npmjs.com/package/@swaggerexpert/json-pointer)
[![Test workflow](https://github.com/swaggerexpert/json-pointer/actions/workflows/test.yml/badge.svg)](https://github.com/swaggerexpert/json-pointer/actions)
[![Dependabot enabled](https://img.shields.io/badge/Dependabot-enabled-blue.svg)](https://dependabot.com/)
[![try on RunKit](https://img.shields.io/badge/try%20on-RunKit-brightgreen.svg?style=flat)](https://npm.runkit.com/@swaggerexpert/json-pointer)
[![](https://data.jsdelivr.com/v1/package/npm/@swaggerexpert/json-pointer/badge)](https://www.jsdelivr.com/package/npm/@swaggerexpert/json-pointer)
[![Tidelift](https://tidelift.com/badges/package/npm/@swaggerexpert%2Fjson-pointer)](https://tidelift.com/subscription/pkg/npm-.swaggerexpert-json-pointer?utm_source=npm-swaggerexpert-json-pointer&utm_medium=referral&utm_campaign=readme)

`@swaggerexpert/json-pointer` is a **parser**, **validator**, **evaluator**, **compiler** and **representer** for [RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901) JavaScript Object Notation (JSON) Pointer.


<table>
  <tr>
    <td align="right" valign="middle">
        <img src="https://cdn2.hubspot.net/hubfs/4008838/website/logos/logos_for_download/Tidelift_primary-shorthand-logo.png" alt="Tidelift" width="60" />
      </td>
      <td valign="middle">
        <a href="https://tidelift.com/subscription/pkg/npm-.swaggerexpert-json-pointer?utm_source=npm-swaggerexpert-json-pointer&utm_medium=referral&utm_campaign=readme">
            Get professionally supported @swaggerexpert/json-pointer with Tidelift Subscription.
        </a>
      </td>
  </tr>
</table>

## Table of Contents

- [Getting started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
    - [Parsing](#parsing)
      - [Translators](#translators)
        - [CST](#cst-translator)
        - [AST](#ast-translator)
        - [XML](#xml-translator)
      - [Statistics](#statistics)
      - [Tracing](#tracing)
    - [Validation](#validation)
    - [Escaping](#escaping)
    - [Evaluation](#evaluation)
      - [Strict Arrays](#strict-arrays)
      - [Strict Objects](#strict-objects)
      - [Evaluation Realms](#evaluation-realms)
        - [JSON](#json-evaluation-realm)
        - [Map/Set](#mapset-evaluation-realm)
        - [Minim](#minim-evaluation-realm)
        - [ApiDOM](#apidom-evaluation-realm)
        - [Immutable.js](#immutablejs-evaluation-realm)
        - [Custom](#custom-evaluation-realms)
        - [Composing Realms](#composing-evaluation-realms)
      - [Evaluation Diagnostics](#evaluation-diagnostics)
      - [Evaluation Tracing](#evaluation-tracing)
    - [Compilation](#compilation)
    - [Representation](#representation)
      - [JSON String](#json-string)
      - [URI Fragment Identifier](#uri-fragment-identifier)
    - [Errors](#errors)
    - [Grammar](#grammar)
- [More about JSON Pointer](#more-about-json-pointer)
- [License](#license)


## Getting started

### Installation

You can install `@swaggerexpert/json-pointer` using `npm`:

```sh
 $ npm install @swaggerexpert/json-pointer
```

---

**[unpkg.com](https://unpkg.com/)**

Include following script tag into your HTML file:

```html
<script src="https://unpkg.com/@swaggerexpert/json-pointer@latest/dist/json-pointer.browser.min.js"></script>
```

Global variable `JSONPointer` will be available in the browser matching interface of `@swaggerexpert/json-pointer` npm package.
`json-pointer.browser.min.js` is [UMD](https://github.com/umdjs/umd?tab=readme-ov-file#umd-universal-module-definition) minified build artifact.
There is also unminified `json-pointer.browser.js` artifact, suitable for debugging.

---

**[jsDelivr](https://www.jsdelivr.com/)**

Include following script tag into your HTML file (ESM mode):

```html
<script type="module">
  import * as JSONPointer from 'https://cdn.jsdelivr.net/npm/@swaggerexpert/json-pointer@latest/+esm'
</script>
```
Local variable `JSONPointer` will be available in scope of the `<script>` tag matching interface of `@swaggerexpert/json-pointer` package.

Or include following script tag into your HTML file (UMD mode):

```html
<script src="https://cdn.jsdelivr.net/npm/@swaggerexpert/json-pointer@latest/dist/json-pointer.browser.min.js"></script>
```

Global variable `JSONPointer` will be available in the browser matching interface of `@swaggerexpert/json-pointer` npm package.
`json-pointer.browser.min.js` is [UMD](https://github.com/umdjs/umd?tab=readme-ov-file#umd-universal-module-definition) minified build artifact.
There is also unminified `json-pointer.browser.js` artifact, suitable for debugging.

### Usage

`@swaggerexpert/json-pointer` currently supports **parsing**, **validation** ,**evaluation**, **compilation** and **representation**.
Both parser and validator are based on a superset of [ABNF](https://www.rfc-editor.org/rfc/rfc5234) ([SABNF](https://cs.github.com/ldthomas/apg-js2/blob/master/SABNF.md))
and use [apg-lite](https://github.com/ldthomas/apg-lite) parser generator.

#### Parsing

Parsing a JSON Pointer is as simple as importing the **parse** function and calling it.

```js
import { parse } from '@swaggerexpert/json-parse';

const parseResult = parse('/foo/bar');
```

**parseResult** variable has the following shape:

```
{
  result: <ParseResult['result]>,
  tree: <ParseResult['tree']>,
  stats: <ParseResult['stats']>,
  trace: <ParseResult['trace']>,
}
```

[TypeScript typings](https://github.com/swaggerexpert/json-pointer/blob/main/types/index.d.ts) are available for all fields attached to parse result object returned by the `parse` function.

##### Translators

`@swaggerexpert/json-pointer` provides several translators to convert the parse result into different tree representations.

###### CST translator

[Concrete Syntax Tree](https://en.wikipedia.org/wiki/Parse_tree) (Parse tree) representation is available on parse result
when instance of `CSTTranslator` is provided via a `translator` option to the `parse` function.
CST is suitable to be consumed by other tools like IDEs, editors, etc...

```js
import { parse, CSTTranslator } from '@swaggerexpert/json-pointer';

const { tree: CST } = parse('/foo/bar', { translator: new CSTTranslator() });
```

CST tree has a shape documented by [TypeScript typings (CSTTree)](https://github.com/swaggerexpert/json-pointer/blob/main/types/index.d.ts).

###### AST translator

**Default translator**. [Abstract Syntax Tree](https://en.wikipedia.org/wiki/Abstract_syntax_tree) representation is available on parse result
by default or when instance of `ASTTranslator` is provided via a `translator` option to the `parse` function.
AST is suitable to be consumed by implementations that need to analyze the structure of the JSON Pointer
or for building a custom JSON Pointer evaluation engine.

AST of the parsed JSON Pointer is a list of unescaped reference tokens.

```js
import { parse } from '@swaggerexpert/json-pointer';

const { tree: AST } = parse('/foo/bar'); // AST = ['foo', 'bar']
```

or

```js
import { parse, ASTTranslator } from '@swaggerexpert/json-poiner';

const { tree: AST } = parse('/foo/bar', { translator: new ASTTranslator() }); // AST = ['foo', 'bar']
```

###### XML translator

```js
import { parse, XMLTranslator } from '@swaggerexpert/json-pointer';

const { tree: XML } = parse('$.store.book[0].title', { translator: new XMLTranslator() });
```

##### Statistics

`parse` function returns additional statistical information about the parsing process.
Collection of the statistics can be enabled by setting `stats` option to `true`.

```js
import { parse } from '@swaggerexpert/json-pointer';

const { stats } = parse('/foo/bar', { stats: true });

stats.displayStats(); // returns operator stats
stats.displayHits(); // returns rules grouped by hit count
```

##### Tracing

`parse` function returns additional tracing information about the parsing process.
Tracing can be enabled by setting `trace` option to `true`. Tracing is essential
for debugging failed matches or analyzing rule execution flow.

```js
import { parse } from '@swaggerexpert/json-pointer';

const { result, trace } = parse('1', { trace: true });

result.success; // returns false
trace.displayTrace(); // returns trace information
trace.inferExpectations(); // returns parser expectations
```

By combining information from `result` and `trace`, it is possible to analyze the parsing process in detail
and generate a messages like this: `'Invalid JSON Pointer: "1". Syntax error at position 0, expected "/"'`. Please see this
[test file](https://github.com/swaggerexpert/json-pointer/blob/main/test/parse/trace.js) for more information how to achieve that.

#### Validation

Validating a JSON Pointer is as simple as importing one of the validation functions and calling it.

```js
import {
  testJSONPointer,
  testReferenceToken,
  testArrayLocation,
  testArrayIndex,
  testArrayDash
} from '@swaggerexpert/json-pointer';

testJSONPointer('/foo/bar'); // => true
testReferenceToken('foo'); // => true
testArrayLocation('0'); // => true
testArrayLocation('-'); // => true
testArrayIndex('0'); // => true
testArrayDash('-'); // => true
```

#### Escaping

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

Because the characters `'~'` (%x7E) and `'/'` (%x2F) have special
meanings in JSON Pointer, `'~'` needs to be encoded as `'~0'` and `'/'`
needs to be encoded as `'~1'` when these characters appear in a
reference token.

```js
import { escape } from '@swaggerexpert/json-pointer';

escape('~foo'); // => '~0foo'
escape('/foo'); // => '~1foo'
```

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

Unescape is performed by first transforming any
occurrence of the sequence `'~1'` to `'/'`, and then transforming any
occurrence of the sequence `'~0'` to `'~'`.  By performing the
substitutions in this order, this library avoids the error of
turning `'~01'` first into `'~1'` and then into `'/'`, which would be
incorrect (the string `'~01'` correctly becomes `'~1'` after transformation).

```js
import { unescape } from '@swaggerexpert/json-pointer';

unescape('~0foo'); // => '~foo'
unescape('~1foo'); // => '/foo'
```

#### Evaluation

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

Evaluation of a JSON Pointer begins with a reference to the root
value of a JSON document and completes with a reference to some value
within the document. Each reference token in the JSON Pointer is
evaluated sequentially.

```js
import { evaluate } from '@swaggerexpert/json-pointer';

const value = {
  "foo": ["bar", "baz"],
  "": 0,
  "a/b": 1,
  "c%d": 2,
  "e^f": 3,
  "g|h": 4,
  "i\\j": 5,
  "k\"l": 6,
  " ": 7,
  "m~n": 8
};

evaluate(value, ''); // => identical to value
evaluate(value, '/foo'); // => ["bar", "baz"]
evaluate(value, '/foo/0'); // => "bar"
evaluate(value, '/'); // => 0
evaluate(value, '/a~1b'); // => 1
evaluate(value, '/c%d'); // => 2
evaluate(value, '/e^f'); // => 3
evaluate(value, '/g|h'); // => 4
evaluate(value, '/i\\j'); // => 5
evaluate(value, '/k"l'); // => 6
evaluate(value, '/ '); // => 7
evaluate(value, '/m~0n'); // => 8

// neither object nor array
evaluate(null, '/foo'); // => throws JSONPointerTypeError
// arrays
evaluate(value, '/foo/2'); // => throws JSONPointerIndexError
evaluate(value, '/foo/-'); // => throws JSONPointerIndexError
evaluate(value, '/foo/a'); // => throws JSONPointerIndexError
// objects
evaluate(value, '/bar'); // => throws JSONPointerKeyError
```

###### Strict Arrays

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

By default, the evaluation is **strict**, meaning an error condition will be raised if it fails to
resolve a concrete value for any of the JSON pointer's reference tokens. For example, if an array
is referenced with a non-numeric token, an error condition will be raised.

Note that the use of the `"-"` character to index an array will always
result in such an error condition because by definition it refers to
a nonexistent array element.

This spec compliant strict behavior can be disabled by setting the `strictArrays` option to `false`.

```js
evaluate(value, '/foo/2', { strictArrays: false }); // => undefined
evaluate(value, '/foo/-', { strictArrays: false }); // => undefined
evaluate(value, '/foo/132423', { strictArrays: false }); // => undefined
```

###### Strict Objects

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

By default, the evaluation is **strict**, meaning error condition will be raised if it fails to
resolve a concrete value for any of the JSON pointer's reference tokens. For example,
if a token references a key that is not present in an object, an error condition will be raised.

This spec compliant strict behavior can be disabled by setting the `strictObjects` option to `false`.

```js
evaluate(value, '/bar', { strictObjects: false }); // => undefined
```

`strictObjects` options has no effect in cases where evaluation of previous
reference token failed to resolve a concrete value.

```js
evaluate(value, '/bar/baz', { strictObjects: false }); // => throw JSONPointerTypeError
```

##### Evaluation Realms

An **evaluation realm** defines the rules for interpreting and navigating data structures in JSON Pointer evaluation.
While JSON Pointer traditionally operates on JSON objects and arrays, evaluation realms allow the evaluation to work
polymorphically with different data structures, such as [Map](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Map),
[Set](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Set), [Immutable.js](https://immutable-js.com/),
or even custom representations like [ApiDOM](https://github.com/swagger-api/apidom).
Realm can be specified via the `realm` option in the `evalute()` function.

###### JSON Evaluation Realm

By default, the evaluation operates under the **JSON realm**, which assumes that:

- **Arrays** are indexed numerically.
- **Objects** (plain JavaScript objects) are accessed by string keys.

The default realm is represented by the `JSONEvaluationRealm` class.

```js
import { evaluate } from '@swaggerexpert/json-pointer';

evaluate({ a: 'b' }, '/a'); // => 'b'
```

is equivalent to:

```js
import { evaluate } from '@swaggerexpert/json-pointer';
import JSONEvaluationRealm from '@swaggerexpert/json-pointer/evaluate/realms/json';

evaluate({ a: 'b' }, '/a', { realm: new JSONEvaluationRealm() }); // => 'b'
```

###### Map/Set Evaluation Realm

The Map/Set realm extends JSON Pointer evaluation to support [Map](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Map) and [Set](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Set) instances,
allowing structured traversal and access beyond traditional JavaScript objects and arrays.
Map/Set realm is represented by the `MapSetEvaluationRealm` class.


```js
import { evaluate } from '@swaggerexpert/json-pointer';
import MapSetEvaluationRealm from '@swaggerexpert/json-pointer/evaluate/realms/map-set';

const map = new Map([
  ['a', new Set(['b', 'c'])]
]);

evaluate(map, '/a/1', { realm: new MapSetEvaluationRealm() }); // => 'c'
```

###### Minim Evaluation Realm

The Minim Evaluation Realm extends JSON Pointer evaluation to support `minim` data structures,
specifically `ObjectElement`, `ArrayElement`, and other element types from the [minim](https://github.com/refractproject/minim).

Minim is widely used in API description languages (e.g., OpenAPI, API Blueprint, AsyncAPI and other API Description processing tools)
to represent structured API data. The Minim Evaluation Realm enables seamless JSON Pointer traversal for these structures.

Before using the Minim Evaluation Realm, you need to install the `minim` package:

```sh
 $ npm install --save minim
```

```js
import { ObjectElement } from 'minim';
import { evaluate } from '@swaggerexpert/json-pointer';
import MinimEvaluationRealm from '@swaggerexpert/json-pointer/evaluate/realms/minim';

const objectElement = new ObjectElement({
  a: ['b', 'c']
});

evaluate(objectElement, '/a/1', { realm: new MinimEvaluationRealm() }); // => StringElement('c')
```

###### ApiDOM Evaluation Realm

The [ApiDOM](https://github.com/swagger-api/apidom) Evaluation Realm is an integration layer that enables
evaluation of JSON Pointer expressions on ApiDOM structures. It provides compatibility with ApiDOM [core](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core) and namespace packages (`@swagger-api/apidom-ns-*`),
allowing to traverse and query ApiDOM element instances.

Before using the ApiDOM Evaluation Realm, you need to install the `@swagger-api/apidom-core` package:

```sh
 $ npm install --save @swagger-api/apidom-core
```

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { evaluate } from '@swaggerexpert/json-pointer';
import ApiDOMEvaluationRealm from '@swaggerexpert/json-pointer/evaluate/realms/apidom';

const objectElement = new ObjectElement({
  a: ['b', 'c']
});

evaluate(objectElement, '/a/1', { realm: new ApiDOMEvaluationRealm() }); // => StringElement('c')
```

or using contextual evaluation:

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { evaluate } from '@swaggerexpert/json-pointer/evaluate/realms/apidom';

const objectElement = new ObjectElement({
  a: ['b', 'c']
});

evaluate(objectElement, '/a/1'); // => StringElement('c')
```

###### Immutable.js Evaluation Realm

The [Immutable.js](https://immutable-js.com/) Evaluation Realm is an integration layer that enables
evaluation of JSON Pointer expressions on Immutable.js structures.

Before using the Immutable.js Evaluation Realm, you need to install the `immutable` package:

```sh
 $ npm install --save immutable
```

```js
import { fromJS } from 'immutable';
import { evaluate } from '@swaggerexpert/json-pointer';
import ImmutableEvaluationRealm from '@swaggerexpert/json-pointer/evaluate/realms/immutable';

const map = fromJS({
  a: ['b', 'c']
});

evaluate(map, '/a/1', { realm: new ImmutableEvaluationRealm() }); // => 'c'
```


###### Custom Evaluation Realms

The evaluation is designed to support **custom evaluation realms**,
enabling JSON Pointer evaluation for **non-standard data structures**.

A valid custom evaluation realm must match the structure of the [EvaluationRealm interface](https://github.com/swaggerexpert/json-pointer/blob/main/types/index.d.ts).

One way to create a custom realm is to extend the `EvaluationRealm` class and implement the required methods.

```js
import { evaluate, EvaluationRealm } from '@swaggerexpert/json-pointer';

class CustomEvaluationRealm extends EvaluationRealm {
  name = 'cusotm';

  isArray(node) { ... }
  isObject(node) { ... }
  sizeOf(node) { ... }
  has(node, referenceToken) { ... }
  evaluate(node, referenceToken) { ... }
}

evaluate({ a: 'b' }, '/a', { realm: new CustomEvaluationRealm() }); // => 'b'
```

###### Composing Evaluation Realms

Evaluation realms can be composed to create complex evaluation scenarios,
allowing JSON Pointer evaluation to work across multiple data structures in a seamless manner.
By combining different realms, composite evaluation ensures that a JSON Pointer query can
resolve correctly whether the data structure is an object, array, Map, Set, or any custom type.

When composing multiple evaluation realms, the **order matters**. The composition is performed from left to right, meaning:

- More specific realms should be placed first (leftmost position).
- More generic realms should be placed later (rightmost position).

This ensures that specialized data structures (e.g., Map, Set, Immutable.js) take precedence over generic JavaScript objects and arrays.

```js
import { composeRealms, evaluate } from '@swaggerexpert/json-pointer';
import JSONEvaluationRealm from '@swaggerexpert/json-pointer/realms/json';
import MapSetEvaluationRealm from '@swaggerexpert/json-pointer/realms/map-set';

const compositeRealm = composeRealms(new MapSetEvaluationRealm(), new JSONEvaluationRealm());

const structure = [
  {
    a: new Map([
      ['b', new Set(['c', 'd'])]
    ]),
  },
];

evaluate(structure, '/0/a/b/1', { realm : compositeRealm }); // => 'd'
```

##### Evaluation Diagnostics

`@swaggerexpert/json-pointer` provides rich diagnostic information to help identify and resolve issues during JSON Pointer evaluation.

When evaluation fails, the library throws [errors](#errors) from a well-defined hierarchy — all extending from `JSONPointerEvaluateError`.
These errors carry detailed diagnostic metadata describing what failed, where it failed, and why.

Each error includes following fields:

- `jsonPointer` – the full pointer being evaluated
- `referenceToken` – the token that caused the failure
- `referenceTokenPosition` – the index of that token within the pointer
- `referenceTokens` – the full list of parsed reference tokens
- `currentValue` – the value being evaluated at the point of failure
- `realm` – the name of the evaluation realm (e.g., "json")

##### Evaluation Tracing

`@swaggerexpert/json-pointer` package supports evaluation tracing, allowing you to inspect each step of JSON Pointer evaluation in detail.
This is especially useful for debugging, error reporting, visualization tools, or implementing custom behavior like fallbacks and partial evaluations.

How it works?

To enable tracing, provide an empty trace object when calling evaluate.
`trace` object is populated with detailed information about each step of the evaluation process:

Tracing `successful` evaluation:

```js
import { evaluate } from '@swaggerexpert/json-pointer';

const trace = {};
evaluate({ a: 'b' }, '/a', { trace });
```

```js
// trace
{
  steps: [
    {
      referenceToken: 'a',
      referenceTokenPosition: 0,
      input: { a: 'b' },
      inputType: 'object',
      output: 'b',
      success: true
    }
  ],
  failed: false,
  failedAt: -1,
  message: 'JSON Pointer "/a" successfully evaluated against the provided value',
  context: {
    jsonPointer: '/a',
    referenceTokens: [ 'a' ],
    strictArrays: true,
    strictObjects: true,
    realm: 'json',
    value: { a: 'b' }
  }
}
```

Tracing `failed` evaluation:

```js
import { evaluate } from '@swaggerexpert/json-pointer';

const trace = {};
try {
  evaluate({ a: 'b' }, '/c', { trace });
} catch {}
```

```js
// trace
{
  steps: [
    {
      referenceToken: 'c',
      referenceTokenPosition: 0,
      input: { a: 'b' },
      inputType: 'object',
      output: undefined,
      success: false,
      reason: 'Invalid object key "c" at position 0 in "/c": key not found in object'
    }
  ],
  failed: true,
  failedAt: 0,
  message: 'Invalid object key "c" at position 0 in "/c": key not found in object',
  context: {
  jsonPointer: '/c',
    referenceTokens: [ 'c' ],
    strictArrays: true,
    strictObjects: true,
    realm: 'json',
    value: { a: 'b' }
  }
}
```

`trace` option can additionally be set to `true` to enable parse tracing without providing an empty object.
When parsing of the JSON Pointer fails during evaluation, the generated error will contain additional information explaining the failure.
This is the default behavior of the `trace` option. You can also set `trace` to `false` to disable tracing entirely.

#### Compilation

Compilation is the process of transforming a list of unescaped reference tokens into a JSON Pointer.
Reference tokens are escaped before compiled into a JSON Pointer.

```js
import { compile } from '@swaggerexpert/json-pointer';

compile(['~foo', 'bar']); // => '/~0foo/bar'
```

#### Representation

##### JSON String

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

A JSON Pointer can be represented in a JSON string value. Per
[RFC4627, Section 2.5](https://datatracker.ietf.org/doc/html/rfc4627#section-2.5), all instances of quotation mark `'"'` (%x22),
reverse solidus `'\'` (%x5C), and control (%x00-1F) characters MUST be
escaped.

```js
import { JSONString } from '@swaggerexpert/json-pointer';

JSONString.to('/foo"bar'); // => '"/foo\\"bar"'
JSONString.from('"/foo\\"bar"'); // => '/foo"bar'
```

##### URI Fragment Identifier

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

A JSON Pointer can be represented in a URI fragment identifier by
encoding it into octets using UTF-8 [RFC3629](https://datatracker.ietf.org/doc/html/rfc3629), while percent-encoding
those characters not allowed by the fragment rule in [RFC3986](https://datatracker.ietf.org/doc/html/rfc3986#section-3.5).

```js
import { URIFragmentIdentifier } from '@swaggerexpert/json-pointer';

URIFragmentIdentifier.to('/foo"bar'); // => '#/foo%22bar'
URIFragmentIdentifier.from('#/foo%22bar'); // => '/foo"bar'
URIFragmentIdentifier.fromURIReference('https://swaggerexpert.com/path#/foo%22bar'); // => '/foo"bar'
```

#### Errors

`@swaggerexpert/json-pointer` provides a structured error class hierarchy,
enabling precise error handling across JSON Pointer operations, including parsing, evaluation, compilation and validation.

```js
import {
  JSONPointerError,
  JSONPointerParseError,
  JSONPointerCompileError,
  JSONPointerEvaluateError,
  JSONPointerTypeError,
  JSONPointerKeyError,
  JSONPointerIndexError
} from '@swaggerexpert/json-pointer';
```

**JSONPointerError** is the base class for all JSON Pointer errors.

#### Grammar

New grammar instance can be created in following way:

```js
import { Grammar } from '@swaggerexpert/json-pointer';

const grammar = new Grammar();
```

To obtain original ABNF (SABNF) grammar as a string:

```js
import { Grammar } from '@swaggerexpert/json-pointer';

const grammar = new Grammar();

grammar.toString();
// or
String(grammar);
```

## More about JSON Pointer

JSON Pointer is defined by the following [ABNF](https://tools.ietf.org/html/rfc5234) syntax

[comment]: <> (SPDX-FileCopyrightText: Copyright &#40;c&#41; 2013 IETF Trust and the persons identified as the document authors.  All rights reserved.)
[comment]: <> (SPDX-License-Identifier: BSD-2-Clause)

```abnf
; JavaScript Object Notation (JSON) Pointer ABNF syntax
; https://datatracker.ietf.org/doc/html/rfc6901
json-pointer    = *( slash reference-token ) ; MODIFICATION: surrogate text rule used
reference-token = *( unescaped / escaped )
unescaped       = %x00-2E / %x30-7D / %x7F-10FFFF
                ; %x2F ('/') and %x7E ('~') are excluded from 'unescaped'
escaped         = "~" ( "0" / "1" )
                ; representing '~' and '/', respectively

; https://datatracker.ietf.org/doc/html/rfc6901#section-4
array-location  = array-index / array-dash
array-index     = %x30 / ( %x31-39 *(%x30-39) )
                ; "0", or digits without a leading "0"
array-dash      = "-"

; Surrogate named rules
slash           = "/"
```

## License

`@swaggerexpert/json-pointer` is licensed under [Apache 2.0 license](https://github.com/swaggerexpert/json-pointer/blob/main/LICENSE).
`@swaggerexpert/json-pointer` comes with an explicit [NOTICE](https://github.com/swaggerexpert/json-pointer/blob/main/NOTICE) file
containing additional legal notices and information.
