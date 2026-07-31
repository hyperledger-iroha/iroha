# @swagger-api/apidom-parser-adapter-api-design-systems-yaml

`@swagger-api/apidom-parser-adapter-api-design-systems-yaml` is a parser adapter for [API Design Systems](https://apidesign.systems/) specification versions defined in [YAML format](https://yaml.org/spec/1.2/spec.html).

Supported versions:
- 2021-05-07

Under the hood this adapter uses [@swagger-api/apidom-parser-adapter-yaml-1-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-yaml-1-2)
to parse a source string into generic ApiDOM in [base ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core#base-namespace)
which is then refracted with [API Design Systems Refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-api-design-systems#refractors).

## Installation

After [prerequisites](https://github.com/swagger-api/apidom/blob/main/README.md#prerequisites) for installing this package are satisfied, you can install it
via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-parser-adapter-api-design-systems-yaml
```

## Parser adapter API

This parser adapter is fully compatible with parser adapter interface required by [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#mounting-parser-adapters)
and implements all required properties.

### mediaTypes

Defines list of media types that this parser adapter recognizes.

```js
[
  'application/vnd.aai.apidesignsystems;version=2021-05-07',
  'application/vnd.aai.apidesignsystems+yaml;version=2021-05-07'
]
```

### detect

[Detection](https://github.com/swagger-api/apidom/blob/main/packages/apidom-parser-adapter-api-design-systems-yaml/src/adapter.ts#L21) is based on a regular expression matching required API Design Systems specification symbols in YAML format.

### namespace

This adapter exposes an instance of [API Design Systems ApiDOM namespace](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-api-design-systems#api-design-systems-2021-05-07-namespace).

### parse

`parse` function consumes various options as a second argument. Here is a list of these options:

Option | Type | Default | Description
--- | --- | --- | ---
<a name="specObj"></a>`specObj` | `Object` | [Specification Object](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-api-design-systems/src/refractor/specification.ts) | This specification object drives the JSON AST transformation to API Design Systems ApiDOM namespace.
<a name="sourceMap"></a>`sourceMap` | `Boolean` | `false` | Indicate whether to generate source maps.
<a name="refractorOpts"></a>`refractorOpts` | `Object` | `{}` | Refractor options are [passed to refractors](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-api-design-systems#refractor-plugins) during refracting phase.

All unrecognized arbitrary options will be ignored.

## Usage

This parser adapter can be used directly or indirectly via [@swagger-api/apidom-parser](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser).

### Direct usage

During direct usage you don't need to provide `mediaType` as the `parse` function is already pre-bound
with [supported media types](#mediatypes).

```js
import { parse, detect } from '@swagger-api/apidom-parser-adapter-api-design-systems-json';

// detecting
await detect('version: "2021-05-07"'); // => true
await detect('test'); // => false

// parsing
const parseResult = await parse('version: "2021-05-07"', { sourceMap: true });
```

### Indirect usage

You can omit the `mediaType` option here, but please read [Word on detect vs mediaTypes](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser#word-on-detect-vs-mediatypes) before you do so.

```js
import ApiDOMParser from '@swagger-api/apidom-parser';
import * as apiDesignSystemsYamlAdapter from '@swagger-api/apidom-parser-adapter-api-design-systems-yaml';

const parser = new ApiDOMParser();

parser.use(apiDesignSystemsYamlAdapter);

const parseResult = await parser.parse('version: "2021-05-07"', {
  mediaType: apiDesignSystemsYamlAdapter.mediaTypes.latest('json')
});
```
