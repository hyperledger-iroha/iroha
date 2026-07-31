# @swagger-api/apidom-json-pointer

`apidom-json-pointer` is a package that evaluates [JSON Pointer](https://datatracker.ietf.org/doc/html/rfc6901) against ApiDOM.

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-json-pointer
```

## Modern API

This is the recommended API for use in new projects. It is fully compliant with [RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901) and supports all aspects of JSON Pointer.
Uses [@swaggerexpert/json-pointer](https://www.npmjs.com/package/@swaggerexpert/json-pointer) under the hood and fully reflects its API. For additional options and details, refer to the `@swaggerexpert/json-pointer` [documentation](https://www.npmjs.com/package/@swaggerexpert/json-pointer#usage).

Evaluation is contextual to [ApiDOM realm](https://github.com/swaggerexpert/json-pointer?tab=readme-ov-file#apidom-evaluation-realm) - meaning `evaluate` function
expects only ApiDOM as the first argument.

```js
import { evaluate } from '@swagger-api/apidom-json-pointer/modern';
```

### Evaluating

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { evaluate } from '@swagger-api/apidom-json-pointer/modern';

const apidom = new ObjectElement({ a: { b: 'c' } });
const result =  evaluate(apidom, '/a/b');
// => StringElement('c')
```

### Parsing

Parses JSON Pointer into a list of tokens, which can be accessed through the `tree` property of the parse result.

```js
import { parse } from '@swagger-api/apidom-json-pointer/modern';

const parseResult = parse('/a/b');
// =>
// {
//   result: {
//     success: true,
//     state: 101,
//     stateName: 'MATCH',
//     length: 4,
//     matched: 4,
//     maxMatched: 4,
//     maxTreeDepth: 8,
//     nodeHits: 31
//   },
//   tree: [ 'a', 'b' ],
//   stats: undefined,
//   trace: undefined
// }
```

### Compiling

Compiles a list of tokens into JSON Pointer.

```js
import { compile } from '@swagger-api/apidom-json-pointer/modern';

const jsonPointer = compile(['a', 'b']); // => '/a/b'
```

### Escaping

Escapes/unescapes tokens of JSON Pointer.

```js
import { escape, unescape } from '@swagger-api/apidom-json-pointer/modern';

escape('~a/'); // => '~0a~1'
unescape('~0a~1'); // => '~a/'
```

### Transforming URI to JSON Pointer

Handles case of [URI Fragment Identifier Representation](https://datatracker.ietf.org/doc/html/rfc6901#section-6).

```js
import { URIFragmentIdentifier } from '@swagger-api/apidom-json-pointer/modern';

URIFragmentIdentifier.fromURIReference('https://example.com/path/#/a/b'); // => '/a/b'
```

### Validating

Validates a JSON Pointer and its tokens.

```js
import {
  testJSONPointer,
  testReferenceToken,
  testArrayLocation,
  testArrayIndex,
  testArrayDash,
} from '@swagger-api/apidom-json-pointer/modern';

testJSONPointer('/a/b'); // => true
testReferenceToken('a'); // => true
testArrayLocation('0'); // => true
testArrayLocation('-'); // => true
testArrayIndex('0'); // => true
testArrayDash('-'); // => true
```

### Invalid JSON Pointers

`JSONPointerError` is the base class for all JSON Pointer errors.

```js
import { JSONPointerError } from '@swagger-api/apidom-json-pointer/modern';
```

If an invalid list of tokens is supplied to `compile` function, `JSONPointerCompileError` is thrown.

```js
import { JSONPointerCompileError } from '@swagger-api/apidom-json-pointer/modern';
```

If an invalid JSON Pointer is supplied to `evaluate` function, `JSONPointerEvaluateError` is thrown.

```js
import { JSONPointerEvaluateError } from '@swagger-api/apidom-json-pointer/modern';
```

If a valid JSON Pointer is supplied to `evaluate` function and the pointer cannot be evaluated against ApiDOM fragment because it is not an object or an array, `JSONPointerTypeError` is thrown.

```js
import { JSONPointerTypeError } from '@swagger-api/apidom-json-pointer/modern';
```

If a valid JSON Pointer is supplied to `evaluate` function and the pointer cannot be evaluated against ApiDOM fragment because the key does not exist in the object, `JSONPointerKeyError` is thrown.

```js
import { JSONPointerKeyError } from '@swagger-api/apidom-json-pointer/modern';
```

If a valid JSON Pointer is supplied to `evaluate` function and the pointer cannot be evaluated against ApiDOM fragment because the index does not exist in the array, `JSONPointerIndexError` is thrown. 

```js
import { JSONPointerIndexError } from '@swagger-api/apidom-json-pointer/modern';
```

If an error occurs in `parse` function, `JSONPointerParseError` is thrown.

```js
import { JSONPointerParseError } from '@swagger-api/apidom-json-pointer/modern';
```

## Legacy API

This is a legacy API not recommended for use in new projects. It is provided for backward compatibility only.
The legacy API implementation is not RFC 6901 compliant, nor does it support all features of JSON Pointer.

Importing legacy API from `@swagger-api/apidom-json-pointer` is equivalent to importing from `@swagger-api/apidom-json-pointer/legacy`.

```js
import { evaluate } from '@swagger-api/apidom-json-pointer';
```
or
```js
import { evaluate } from '@swagger-api/apidom-json-pointer/legacy';
```

### Evaluating

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { evaluate } from '@swagger-api/apidom-json-pointer';

const apidom = new ObjectElement({ a: { b: 'c' } });
const result =  evaluate('/a/b', apidom);
// => StringElement('c')
```

### Parsing

Parses JSON Pointer into a list of tokens.

```js
import { parse } from '@swagger-api/apidom-json-pointer';

const tokens = parse('/a/b'); // => ['a', 'b']
```

### Compiling

Compiles a list of tokens into JSON Pointer.

```js
import { compile } from '@swagger-api/apidom-json-pointer';

const jsonPointer = compile(['a', 'b']); // => '/a/b'
```

### Escaping

Escapes/unescapes tokens of JSON Pointer.

```js
import { escape, unescape } from '@swagger-api/apidom-json-pointer';

escape('~a/'); // => '~0a~1'
unescape('~0a~1'); // => '~a/'
```

### Transforming URI to JSON Pointer

Handles case of [URI Fragment Identifier Representation](https://datatracker.ietf.org/doc/html/rfc6901#section-6).

```js
import { uriToPointer } from '@swagger-api/apidom-json-pointer';

uriToPointer('https://example.com/path/#/a/b'); // => '/a/b'
```

### Invalid JSON Pointers

If an invalid JSON Pointer is supplied to `parse` or `evaluate` functions, `InvalidJsonPointerError`
is thrown.

```js
import { InvalidJsonPointerError } from '@swagger-api/apidom-json-pointer';
```

If a valid JSON Pointer is supplied to `evaluate` function and the pointer cannot be evaluated against
ApiDOM fragment, `EvaluationJsonPointerError` is thrown.

```js
import { EvaluationJsonPointerError } from '@swagger-api/apidom-json-pointer';
```

