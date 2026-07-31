# @swagger-api/apidom-parser-adapter-json

`@swagger-api/apidom-parser-adapter-json` is a parser adapter for the [JSON format](https://www.json.org/json-en.html).

[CST](https://tree-sitter.github.io/tree-sitter/using-parsers/2-basic-parsing.html#syntax-nodes) produced by lexical analysis is [syntactically analyzed](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-json/src/syntactic-analysis) and
ApiDOM structure using [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace) is produced.

## Installation

After [prerequisites](https://github.com/swagger-api/apidom/blob/main/README.md#prerequisites) for installing this package are satisfied, you can install it
via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-parser-adapter-json
```

## Parse phases

The parse stage takes JSON string and produces ApiDOM structure using [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace).
There are two phases of parsing: **Lexical Analysis** and **Syntactic Analysis**.

### Lexical Analysis

Lexical Analysis will take a JSON string and turn it into a stream of tokens.
[tree-sitter](https://www.npmjs.com/package/tree-sitter) / [web-tree-sitter](https://www.npmjs.com/package/web-tree-sitter) is used as an underlying lexical analyzer.

### Syntactic Analysis

Syntactic Analysis will take a stream of tokens and turn it into an ApiDOM representation.
[CST](https://tree-sitter.github.io/tree-sitter/using-parsers/2-basic-parsing.html#syntax-nodes) produced by lexical analysis is [syntactically analyzed](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-json/src/syntactic-analysis)
and ApiDOM structure using [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace) is produced.

#### [Direct Syntactical analysis](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-json/src/syntactic-analysis/direct)

This analysis directly turns tree-sitter CST into ApiDOM. Single traversal is required which makes
it super performant, and it's the default analysis used.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-json';

const parseResult = await parse('{"prop": "value"}', {
  syntacticAnalysis: 'direct',
});
```

#### [Indirect Syntactic analysis](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-json/src/syntactic-analysis/indirect)

This analysis turns trees-sitter CST into [JSON AST](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ast#json-ast-nodes) representation.
Then JSON AST is turned into ApiDOM. Two traversals are required, which makes indirect analysis less performant than direct one.
Thought less performant, having JSON AST representation allows us to do further complex analysis.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-json';

const parseResult = await parse('{"prop": "value"}', {
  syntacticAnalysis: 'indirect',
});
```

## Parser adapter API

This parser adapter is fully compatible with parser adapter interface required by [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#mounting-parser-adapters)
and implements all required properties.

### mediaTypes

Defines list of media types that this parser adapter recognizes.

```js
['application/json']
```

### detect

[Detection](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-json/src/adapter-node.ts#L23) is based on a regular expression matching valid JSON data types to indicate whether the provided source string is or isn't JSON string.

### namespace

This adapter exposes an instance of [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace).

### parse

`parse` function consumes various options as a second argument. Here is a list of these options:

Option | Type | Default | Description
--- | --- | --- | ---
<a name="sourceMap"></a>`sourceMap` | `Boolean` | `false` | Indicate whether to generate source maps.
<a name="syntacticAnalysis"></a>`syntacticAnalysis` | `String` | `direct` | Indicate type of syntactic analysis

All unrecognized arbitrary options will be ignored.

## Usage

This parser adapter can be used directly or indirectly via [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser).

### Direct usage

During direct usage you don't need to provide `mediaType` as the `parse` function is already pre-bound
with [supported media types](#mediatypes).

```js
import { parse, detect } from '@swagger-api/apidom-parser-adapter-json';

// detecting
await detect('{"prop": "value"}'); // => true
await detect('test'); // => false

// parsing
const parseResult = await parse('{"prop": "value"}', { sourceMap: true });
```

### Indirect usage

You can omit the `mediaType` option here, but please read [Word on detect vs mediaTypes](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#word-on-detect-vs-mediatypes) before you do so.

```js
import ApiDOMParser from '@swagger-api/apidom-parser';
import * as jsonParserAdapter from '@swagger-api/apidom-parser-adapter-json';

const parser = new ApiDOMParser();

parser.use(jsonParserAdapter);

const parseResult = await parser.parse('{"prop", "value"}', { mediaType: jsonParserAdapter.mediaTypes.latest('json') });
```
