# @swagger-api/apidom-parser-adapter-openapi-json-2

`@swagger-api/apidom-parser-adapter-openapi-json-2` is a parser adapter for the [OpenAPI 2.0 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md) in [JSON format](https://www.json.org/json-en.html).
Under the hood this adapter uses [apidom-parser-adapter-json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-json)
to parse a source string into generic ApiDOM in [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace)
which is then refracted with [OpenAPI 2.0 Refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-2#refractors).

## Installation

After [prerequisites](https://github.com/swagger-api/apidom/blob/main/README.md#prerequisites) for installing this package are satisfied, you can install it
via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-parser-adapter-openapi-json-2
```

## Parser adapter API

This parser adapter is fully compatible with parser adapter interface required by [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#mounting-parser-adapters)
and implements all required properties.

### mediaTypes

Defines list of media types that this parser adapter recognizes.

```js
[
  'application/vnd.oai.openapi;version=2.0',
  'application/vnd.oai.openapi+json;version=2.0',
]
```

### detect

[Detection](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-openapi-json-2/src/adapter.ts#L17) is based on a regular expression matching required OpenAPI 2.0 specification symbols in JSON format.

### namespace

This adapter exposes an instance of [OpenAPI 2.0 ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-2#openapi-20-namespace).

### parse

`parse` function consumes various options as a second argument. Here is a list of these options:

Option | Type | Default | Description
--- | --- | --- | ---
<a name="specObj"></a>`specObj` | `Object` | [Specification Object](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-2/src/refractor/specification.ts) | This specification object drives the JSON AST transformation to OpenAPI 2.0 ApiDOM namespace.
<a name="sourceMap"></a>`sourceMap` | `Boolean` | `false` | Indicate whether to generate source maps.
<a name="refractorOpts"></a>`refractorOpts` | `Object` | `{}` | Refractor options are [passed to refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-2#refractor-plugins) during refracting phase.

All unrecognized arbitrary options will be ignored.

## Usage

This parser adapter can be used directly or indirectly via [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser).

### Direct usage

During direct usage you don't need to provide `mediaType` as the `parse` function is already pre-bound
with [supported media types](#mediatypes).

```js
import { parse, detect } from '@swagger-api/apidom-parser-adapter-openapi-json-2';

// detecting
await detect('{"swagger": "2.0"}'); // => true
await detect('test'); // => false

// parsing
const parseResult = await parse('{"swagger": "2.0"}', { sourceMap: true });
```

### Indirect usage

You can omit the `mediaType` option here, but please read [Word on detect vs mediaTypes](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#word-on-detect-vs-mediatypes) before you do so.

```js
import ApiDOMParser from '@swagger-api/apidom-parser';
import * as openApiJsonAdapter from '@swagger-api/apidom-parser-adapter-openapi-json-2';

const parser = new ApiDOMParser();

parser.use(openApiJsonAdapter);

const parseResult = await parser.parse('{"swagger": "2.0"}', { mediaType: openApiJsonAdapter.mediaTypes.latest('json') });
```
