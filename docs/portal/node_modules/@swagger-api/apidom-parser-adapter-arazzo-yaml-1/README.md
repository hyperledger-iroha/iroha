# @swagger-api/apidom-parser-adapter-arazzo-yaml-1

`@swagger-api/apidom-parser-adapter-arazzo-yaml-1-0` is a parser adapter for the [Arazzo 1.0.1 specification](https://spec.openapis.org/arazzo/latest.html#version-1-0-1) in [YAML format](https://yaml.org/spec/1.2/spec.html).
Under the hood this adapter uses [apidom-parser-adapter-yaml-1-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-yaml-1-2)
to parse a source string into generic ApiDOM in [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace)
which is then refracted with [Arazzo 1.x.y Refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-arazzo-1#refractors).

## Installation

After [prerequisites](https://github.com/swagger-api/apidom/blob/main/README.md#prerequisites) for installing this package are satisfied, you can install it
via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-parser-adapter-arazzo-yaml-1
```

## Parser adapter API

This parser adapter is fully compatible with parser adapter interface required by [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#mounting-parser-adapters)
and implements all required properties.

### mediaTypes

Defines list of media types that this parser adapter recognizes.

```js
[
  'application/vnd.oai.workflows;version=1.0.0',
  'application/vnd.oai.workflows+yaml;version=1.0.0',
  'application/vnd.oai.workflows;version=1.0.1',
  'application/vnd.oai.workflows+yaml;version=1.0.1',
]
```

### detect

[Detection](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-arazzo-json-1/src/adapter.ts#L18) is based on a regular expression matching required Arazzo 1.0.0 specification symbols in YAML format.

### namespace

This adapter exposes an instance of [Arazzo 1.x.y ApiDOM namespace](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-arazzo-1/README.md#arazzo-101-namespace).

### parse

`parse` function consumes various options as a second argument. Here is a list of these options:

Option | Type | Default                                                                                                                            | Description
--- | --- |------------------------------------------------------------------------------------------------------------------------------------| ---
<a name="specObj"></a>`specObj` | `Object` | [Specification Object](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-arazzo-1/src/refractor/specification.ts) | This specification object drives the YAML AST transformation to Arazzo 1.x.y ApiDOM namespace.
<a name="sourceMap"></a>`sourceMap` | `Boolean` | `false`                                                                                                                            | Indicate whether to generate source maps.
<a name="refractorOpts"></a>`refractorOpts` | `Object` | `{}`                                                                                                                               | Refractor options are [passed to refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-arazzo-1#refractor-plugins) during refracting phase.

All unrecognized arbitrary options will be ignored.

## Usage

This parser adapter can be used directly or indirectly via [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser).

### Direct usage

During direct usage you don't need to provide `mediaType` as the `parse` function is already pre-bound
with [supported media types](#mediatypes).

```js
import { parse, detect } from '@swagger-api/apidom-parser-adapter-arazzo-yaml-1';

// detecting
await detect('arazzo: 1.0.0'); // => true
await detect('test'); // => false

// parsing
const parseResult = await parse('arazzo: 1.0.0', { sourceMap: true });
```

### Indirect usage

You can omit the `mediaType` option here, but please read [Word on detect vs mediaTypes](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#word-on-detect-vs-mediatypes) before you do so.

```js
import ApiDOMParser from '@swagger-api/apidom-parser';
import * as arazzoYamlAdapter from '@swagger-api/apidom-parser-adapter-arazzo-yaml-1';

const parser = new ApiDOMParser();

parser.use(arazzoYamlAdapter);

const parseResult = await parser.parse('arazzo: 1.0.0', { mediaType: arazzoYamlAdapter.mediaTypes.latest('yaml') });
```
