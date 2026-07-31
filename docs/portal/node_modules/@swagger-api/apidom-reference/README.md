# @swagger-api/apidom-reference

`@swagger-api/apidom-reference` package contains advanced algorithms for semantic ApiDOM manipulations.
This package is divided into three (3) main components:

- **[Parse component](#parse-component)**
- **[Resolve component](#resolve-component)**
- **[Dereference component](#dereference-component)**
- **[Bundle component](#bundle-component)**

## Installation

After [prerequisites](https://github.com/swagger-api/apidom/blob/main/README.md#prerequisites) for installing this package are satisfied, you can install it
via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-reference
```

## Configurations

This package has two main exports suitable for different use-cases. **Empty** configuration and **saturated** configuration.

### Empty configuration

```js
import { parse } from '@swagger-api/apidom-reference/configuration/empty';
import OpenAPIJSON3_1Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-3-1';

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: [new OpenAPIJSON3_1Parser({ allowEmpty: true, sourceMap: false })]
  }
});
```

When using this approach, `options` object is not configured with parsers, resolvers or strategies.
This is suitable for creating **web bundles** and gives you total control of the contents of your bundles.

### Saturated configuration

```js
import { parse } from '@swagger-api/apidom-reference';
```
or
```js
import { parse } from '@swagger-api/apidom-reference/configuration/saturaged';
```

Both of above imports are equivalent. This approach is suitable for **Node.js** environments.
`options` object is pre-configured with all the parsers, resolvers and strategies.

## Parse component

Parse component consists of implementation of default [parser plugins](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers).
Defaults parser plugin is a specialized wrapper that wraps one of the ApiDOM parser adapter into specialized API.
Standard ApiDOM parser adapter can only parse strings. Parser plugins are capable of parsing local filesystem URIs and network URLs.

**Parsing a file localed on local filesystem:**

```js
import { parse } from '@swagger-api/apidom-reference';

await parse('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' }
});
```

**Parsing an HTTP(S) URL located on internet:**

```js
import { parse } from '@swagger-api/apidom-reference';

await parse('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' }
})
```

Notice how we explicitly pass a `mediaType` parse option. This option is actually **not required**,
but if not provided, the Parse component will try to identify appropriate parser plugin by file contents, and it's extension (`.json`).

What actually happens if you don't provide `mediaType` parse option?

```js
import { parse } from '@swagger-api/apidom-reference';

await parse('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json');
```

The result of this operation is going to be generic ApiDOM structure. By analyzing the name of the file
we can identify the extension of the file as `.json`. At this point we only know
that this file is probably going to contain JSON string, though we have no idea what data (AsyncApi/OpenApi)
is encoded within that JSON string.

In the future, we will introduce smart algorithms for looking in the contents of a file and detecting the
`mediaType` automatically. Of course not explicitly providing `mediaType` has performance implications (running detection)
so providing it is always a better option.

### Parser plugins

Parse component comes with number of default parser plugins.

#### [apidom-json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/apidom-json)

Parses dehydrated ApiDOM structure and hydrates it.
This parser plugin is uniquely identified by `apidom-json` name.

Supported media types are:

```js
[
  'application/vnd.apidom',
  'application/vnd.apidom+json',
]
```

#### [openapi-json-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-json-2)

Wraps [@swagger-api/apidom-parser-adapter-openapi-json-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-json-2) package
and is uniquely  identified by `openapi-json-2` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=2.0',
  'application/vnd.oai.openapi+json;version=2.0',
]
```

#### [openapi-json-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-json-3-0)

Wraps [@swagger-api/apidom-parser-adapter-openapi-json-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-json-3-0) package
and is uniquely  identified by `openapi-json-3-0` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=3.0.0',
  'application/vnd.oai.openapi+json;version=3.0.0',
  'application/vnd.oai.openapi;version=3.0.1',
  'application/vnd.oai.openapi+json;version=3.0.1',
  'application/vnd.oai.openapi;version=3.0.2',
  'application/vnd.oai.openapi+json;version=3.0.2',
  'application/vnd.oai.openapi;version=3.0.3',
  'application/vnd.oai.openapi+json;version=3.0.3',
  'application/vnd.oai.openapi;version=3.0.4',
  'application/vnd.oai.openapi+json;version=3.0.4',
]
```

#### [openapi-yaml-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-yaml-2)

Wraps [@swagger-api/apidom-parser-adapter-openapi-yaml-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-yaml-2) package
and is uniquely  identified by `openapi-yaml-2` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=2.0',
  'application/vnd.oai.openapi+yaml;version=2.0',
]
```

#### [openapi-yaml-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-yaml-3-0)

Wraps [@swagger-api/apidom-parser-adapter-openapi-yaml-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-yaml-3-0) package
and is uniquely  identified by `openapi-yaml-3-0` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=3.0.0',
  'application/vnd.oai.openapi+yaml;version=3.0.0',
  'application/vnd.oai.openapi;version=3.0.1',
  'application/vnd.oai.openapi+yaml;version=3.0.1',
  'application/vnd.oai.openapi;version=3.0.2',
  'application/vnd.oai.openapi+yaml;version=3.0.2',
  'application/vnd.oai.openapi;version=3.0.3',
  'application/vnd.oai.openapi+yaml;version=3.0.3',
  'application/vnd.oai.openapi;version=3.0.4',
  'application/vnd.oai.openapi+yaml;version=3.0.4',
]
```

#### [openapi-json-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-json-3-1)

Wraps [@swagger-api/apidom-parser-adapter-openapi-json-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-json-3-1) package
and is uniquely  identified by `openapi-json-3-1` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=3.1.0',
  'application/vnd.oai.openapi+json;version=3.1.0',
]
```

#### [openapi-yaml-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/openapi-yaml-3-1)

Wraps [@swagger-api/apidom-parser-adapter-openapi-yaml-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-openapi-yaml-3-1) package
and is uniquely  identified by `openapi-yaml-3-1` name.

Supported media types are:

```js
[
  'application/vnd.oai.openapi;version=3.1.0',
  'application/vnd.oai.openapi+yaml;version=3.1.0',
]
```

#### [asyncapi-json-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/asyncapi-json-2)

Wraps [@swagger-api/apidom-parser-adapter-asyncapi-json-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-asyncapi-json-2) package
and is uniquely identified by `asyncapi-json-2` name.

Supported media types are:

```js
[
  'application/vnd.aai.asyncapi;version=2.0.0',
  'application/vnd.aai.asyncapi;version=2.1.0',
  'application/vnd.aai.asyncapi;version=2.2.0',
  'application/vnd.aai.asyncapi;version=2.3.0',
  'application/vnd.aai.asyncapi;version=2.4.0',
  'application/vnd.aai.asyncapi;version=2.5.0',
  'application/vnd.aai.asyncapi;version=2.6.0',
  'application/vnd.aai.asyncapi+json;version=2.0.0',
  'application/vnd.aai.asyncapi+json;version=2.1.0',
  'application/vnd.aai.asyncapi+json;version=2.2.0',
  'application/vnd.aai.asyncapi+json;version=2.3.0',
  'application/vnd.aai.asyncapi+json;version=2.4.0',
  'application/vnd.aai.asyncapi+json;version=2.5.0',
  'application/vnd.aai.asyncapi+json;version=2.6.0',
]
```

#### [asyncapi-yaml-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/asyncapi-yaml-2)

Wraps [@swagger-api/apidom-parser-adapter-asyncapi-yaml-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-asyncapi-yaml-2) package
and is uniquely  identified by `asyncapi-yaml-2` name.


Supported media types are:

```js
[
  'application/vnd.aai.asyncapi;version=2.0.0',
  'application/vnd.aai.asyncapi;version=2.1.0',
  'application/vnd.aai.asyncapi;version=2.2.0',
  'application/vnd.aai.asyncapi;version=2.3.0',
  'application/vnd.aai.asyncapi;version=2.3.0',
  'application/vnd.aai.asyncapi;version=2.4.0',
  'application/vnd.aai.asyncapi;version=2.5.0',
  'application/vnd.aai.asyncapi;version=2.6.0',
  'application/vnd.aai.asyncapi+yaml;version=2.0.0',
  'application/vnd.aai.asyncapi+yaml;version=2.1.0',
  'application/vnd.aai.asyncapi+yaml;version=2.2.0',
  'application/vnd.aai.asyncapi+yaml;version=2.3.0',
  'application/vnd.aai.asyncapi+yaml;version=2.4.0',
  'application/vnd.aai.asyncapi+yaml;version=2.5.0',
  'application/vnd.aai.asyncapi+yaml;version=2.6.0',
]
```

#### [arazzo-json-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/arazzo-json-1)

Wraps [@swagger-api/apidom-parser-adapter-arazzo-json-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-arazzo-json-1) package
and is uniquely identified by `arazzo-json-1` name.

Supported media types are:

```js
[
  'application/vnd.oai.workflows;version=1.0.0',
  'application/vnd.oai.workflows+json;version=1.0.0',
  'application/vnd.oai.workflows;version=1.0.1',
  'application/vnd.oai.workflows+json;version=1.0.1',
]
```

#### [arazzo-yaml-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/arazzo-yaml-1)

Wraps [@swagger-api/apidom-parser-adapter-arazzo-yaml-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-arazzo-yaml-1) package
and is uniquely identified by `arazzo-yaml-1` name.

Supported media types are:

```js
[
  'application/vnd.oai.workflows;version=1.0.0',
  'application/vnd.oai.workflows+yaml;version=1.0.0',
  'application/vnd.oai.workflows;version=1.0.1',
  'application/vnd.oai.workflows+yaml;version=1.0.1',
]
```

#### [api-design-systems-json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/api-design-systems-json)

Wraps [@swagger-api/apidom-parser-adapter-api-design-systsems-json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-api-design-systems-json) package
and is uniquely identified by `api-design-systems-json` name.

Supported media types are:

```js
[
  'application/vnd.aai.apidesignsystems;version=2021-05-07',
  'application/vnd.aai.apidesignsystems+json;version=2021-05-07'
]
```

#### [api-design-systems-yaml](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/api-design-systems-yaml)

Wraps [@swagger-api/apidom-parser-adapter-api-design-systems-yaml](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-api-design-systems-yaml) package
and is uniquely  identified by `api-design-systems-yaml` name.


Supported media types are:

```js
[
  'application/vnd.aai.apidesignsystems;version=2021-05-07',
  'application/vnd.aai.apidesignsystems+yaml;version=2021-05-07'
]
```

#### [json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/json)

Wraps [@swagger-api/apidom-parser-adapter-json](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-json) package
and is uniquely  identified by `json` name.


Supported media types are:

```js
[
  'application/json'
]
```

#### [yaml-1-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/yaml-1-2)

Wraps [@swagger-api/apidom-parser-adapter-yaml-1-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-parser-adapter-yaml-1-2) package
and is uniquely  identified by `yaml-1-2` name.


Supported media types are:

```js
[
  'text/yaml',
  'application/yaml'
]
```

#### [binary](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/parse/parsers/binary)

Can parse any binary or non-binary file and return it's content as `base64` encoded string.
This parser is uniquely identified by `binary` name.


**All** media types are supported.

#### Parser plugins execution order

It's important to understand that default parser plugins are run in specific order. The order is determined
by the [options.parse.parsers](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/configuration/saturated.ts#L35) option.
Every plugin is pulled from `options.parse.parsers` option, and it's `canParse` method is called to determine
whether the plugin can parse the URI. If `canParse` returns `true`, `parse` method of plugin is called
and result from parsing is returned. No subsequent parser plugins are run. If `canParse` returns
`false`, next parser plugin is pulled and this process is repeated until one of the parser plugins `canParse` method
returns `true` or until entire list of parser plugins is exhausted (throws error).

```js
[
  new OpenAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIJSON3_0Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML3_0Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIJSON3_1Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML3_1Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIJSON3Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIYAML3Parser({ allowEmpty: true, sourceMap: false }),
  new ArazzoJSON1Parser({ allowEmpty: true, sourceMap: false }),
  new ArazzoYAML1Parser({ allowEmpty: true, sourceMap: false }),
  new APIDesignSystemsJSONParser({ allowEmpty: true, sourceMap: false }),
  new APIDesignSystemsYAMLParser({ allowEmpty: true, sourceMap: false }),
  new APIDOMJSONParser({ allowEmpty: true, sourceMap: false }),
  new JSONParser({ allowEmpty: true, sourceMap: false }),
  new YAMLParser({ allowEmpty: true, sourceMap: false }),
  new BinaryParser({ allowEmpty: true }),
]
```
Most specific parser plugins are listed first, most generic are listed last.

It's possible to **change** the parser plugins **order globally** by mutating global `parse` options:

```js
import { options } from '@swagger-api/apidom-reference';
import OpenAPIJSON2Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-2';
import OpenAPIYAML2Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-2';
import OpenAPIJSON3_0Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-3-0';
import OpenAPIYAML3_0Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-3-0'
import OpenAPIJSON3_1Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-3-1';
import OpenAPIYAML3_1Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-3-1'
import AsyncAPIJSON2Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-json-2';
import AsyncAPIJSON3Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-json-3';
import AsyncAPIYAML2Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-yaml-2';
import AsyncAPIYAML3Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-yaml-3';
import ArazzoJSON1Parser from '@swagger-api/apidom-reference/parse/parsers/arazzo-json-1';
import ArazzoYAML1Parser from '@swagger-api/apidom-reference/parse/parsers/arazzo-yaml-1';
import APIDOMJSONParser from '@swagger-api/apidom-reference/parse/parsers/apidom-json';
import APIDesignSystemsJSONParser from '@swagger-api/apidom-reference/parse/parsers/api-design-systems-json';
import APIDesignSystemsYAMLParser from '@swagger-api/apidom-reference/parse/parsers/api-design-systems-json';
import JSONParser from '@swagger-api/apidom-reference/parse/parsers/json';
import YAMLParser from '@swagger-api/apidom-reference/parse/parsers/yaml';
import BinaryParser from '@swagger-api/apidom-reference/parse/parsers/binary';


options.parse.parsers = [
  new OpenAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIJSON3_0Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML3_0Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIJSON3_1Parser({ allowEmpty: true, sourceMap: false }),
  new OpenAPIYAML3_1Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIJSON3Parser({ allowEmpty: true, sourceMap: false }),
  new AsyncAPIYAML3Parser({ allowEmpty: true, sourceMap: false }),
  new ArazzoJSON1Parser({ allowEmpty: true, sourceMap: false }),
  new ArazzoYAML1Parser({ allowEmpty: true, sourceMap: false }),
  new APIDesignSystemsJSONParser({ allowEmpty: true, sourceMap: false }),
  new APIDesignSystemsYAMLParser({ allowEmpty: true, sourceMap: false }),
  new APIDOMJSONParser({ allowEmpty: true, sourceMap: false }),
  new YAMLParser({ allowEmpty: true, sourceMap: false }),
  new JSONParser({ allowEmpty: true, sourceMap: false }),
  new BinaryParser({ allowEmpty: true }),
]
```

To **change** the parser plugins **order** on ad-hoc basis:

```js
import { parse } from '@swagger-api/apidom-reference';
import OpenAPIJSON2Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-2';
import OpenAPIYAML2Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-2';
import OpenAPIJSON3_0Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-3-0';
import OpenAPIYAML3_0Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-3-0'
import OpenAPIJSON3_1Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-json-3-1';
import OpenAPIYAML3_1Parser from '@swagger-api/apidom-reference/parse/parsers/openapi-yaml-3-1'
import AsyncAPIJSON2Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-json-2';
import AsyncAPIJSON3Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-json-3';
import AsyncAPIYAML2Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-yaml-2';
import AsyncAPIYAML3Parser from '@swagger-api/apidom-reference/parse/parsers/asyncapi-yaml-3';
import ArazzoJSON1Parser from '@swagger-api/apidom-reference/parse/parsers/arazzo-json-1';
import ArazzoYAML1Parser from '@swagger-api/apidom-reference/parse/parsers/arazzo-yaml-1';
import APIDOMJSONParser from '@swagger-api/apidom-reference/parse/parsers/apidom-json';
import APIDesignSystemsJSONParser from '@swagger-api/apidom-reference/parse/parsers/api-design-systems-json';
import APIDesignSystemsYAMLParser from '@swagger-api/apidom-reference/parse/parsers/api-design-systems-json';
import JSONParser from '@swagger-api/apidom-reference/parse/parsers/json';
import YAMLParser from '@swagger-api/apidom-reference/parse/parsers/yaml';
import BinaryParser from '@swagger-api/apidom-reference/parse/parsers/binary';

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: [
      new OpenAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
      new OpenAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
      new OpenAPIJSON3_0Parser({ allowEmpty: true, sourceMap: false }),
      new OpenAPIYAML3_0Parser({ allowEmpty: true, sourceMap: false }),
      new OpenAPIJSON3_1Parser({ allowEmpty: true, sourceMap: false }),
      new OpenAPIYAML3_1Parser({ allowEmpty: true, sourceMap: false }),
      new AsyncAPIJSON2Parser({ allowEmpty: true, sourceMap: false }),
      new AsyncAPIYAML2Parser({ allowEmpty: true, sourceMap: false }),
      new AsyncAPIJSON3Parser({ allowEmpty: true, sourceMap: false }),
      new AsyncAPIYAML3Parser({ allowEmpty: true, sourceMap: false }),
      new ArazzoJSON1Parser({ allowEmpty: true, sourceMap: false }),
      new ArazzoYAML1Parser({ allowEmpty: true, sourceMap: false }),
      new APIDesignSystemsJSONParser({ allowEmpty: true, sourceMap: false }),
      new APIDesignSystemsYAMLParser({ allowEmpty: true, sourceMap: false }),
      new APIDOMJSONParser({ allowEmpty: true, sourceMap: false }),
      new JSONParser({ allowEmpty: true, sourceMap: false }),
      new YAMLParser({ allowEmpty: true, sourceMap: false }),
      new BinaryParser({ allowEmpty: true }),
    ],
  },
});
```

#### Parser plugin options

Parser plugins accept additional options like `allowEmpty` or `sourceMap`. It's possible to **change** parser plugin
**options globally** by mutating global `parse` options:

```js
import { options, parse } from '@swagger-api/apidom-reference';

options.parser.parserOpts = {
  allowEmpty: false,
  sourceMap: true,
};

await parse('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' }
});
```

To **change** the parser plugins **options** on ad-hoc basis:

```js
import { parse } from '@swagger-api/apidom-reference';

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parserOpts: { allowEmpty: false, sourceMap: true },
  },
});
```

### Creating new parser plugin

Parse component can be extended by additional parser plugins. Every parser plugin is an object that
must conform to the following interface/shape:

```typescript
interface ParserPlugin {
  // uniquely identifies this parser plugin
  readonly name: string;
  allowEmpty: boolean;
  sourceMap: boolean;
  fileExtensions: string[];
  mediaTypes: string[];

  // this method is called to determine whether the parser plugin can parse the file
  canParse(file: File): boolean | Promise<boolean>;

  // this method actually parses the file
  parse(file: File): ParseResultElement | Promise<ParseResultElement>;
}
```

New parser plugin is then provided as an option to a `parse` function:

```ts
import { parse, options, File, Parser, ParserOptions } from '@swagger-api/apidom-reference';

interface MyCustomParserPluginOptions extends Omit<ParserOptions, 'name'> {}

class MyCustomParserPlugin extends Parser {
  constructor(options?: MyCustomParserPluginOptions) {
    super({ ...(options ?? {}), name: 'my-custom-parser' });
  }

  async canParse(file: File, options: typeof options) {
    return true;
  }

  async parse(file: File, options: typeof options) {
    // implementation of parsing
  }
}

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: [...options.parse.parsers, new MyCustomParserPlugin()],
  }
});
```

In this particular example we're adding our custom parser plugin as the last plugin
to the available default parser plugin list, so there's a good chance that one of the
default parser plugins detects that it can parse the `/home/user/oas.json` file,
parses it and returns.

If you want to force execution of your custom plugin, add it as a first parser plugin:

```ts
import { parse, options, File, Parser, ParserOptions } from '@swagger-api/apidom-reference';

interface MyCustomParserPluginOptions extends Omit<ParserOptions, 'name'> {}

class MyCustomParserPlugin extends Parser {
  constructor(options?: MyCustomParserPluginOptions) {
    super({ ...(options ?? {}), name: 'my-custom-parser' });
  }

  async canParse(file: File, options: typeof options) {
    return true;
  }

  async parse(file: File, options: typeof options) {
    // implementation of parsing
  }
}


await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: [new MyCustomParserPlugin(), ...options.parse.parsers],
  }
});
```

To override the default parser plugins entirely, set `myCustomParserPlugin` plugin to be the only one available:

```ts
import { parse, options, File, Parser, ParserOptions } from '@swagger-api/apidom-reference';

interface MyCustomParserPluginOptions extends Omit<ParserOptions, 'name'> {}

class MyCustomParserPlugin extends Parser {
  constructor(options?: MyCustomParserPluginOptions) {
    super({ ...(options ?? {}), name: 'my-custom-parser' });
  }

  async canParse(file: File, options: typeof options) {
    return true;
  }

  async parse(file: File, options: typeof options) {
    // implementation of parsing
  }
}

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: [new MyCustomParserPlugin()],
  }
});
```

### Manipulating parser plugins

Parser plugins can be added, removed, replaced or reordered.

Here are two examples of removing one of the parser plugins called `asyncapi-json-2`.
We're using the fact that every parser plugin is uniquely identifiable by its name.

**Removing** parser plugin **globally** for all subsequence `parse` calls is achieved by mutating global options:

```js
import { parse, options, mergeOptions } from '@swagger-api/apidom-reference';

options.parse.parsers = options.parse.parsers.filter(parserPlugin => parserPlugin !== 'asyncapi-json-2')

// here you can be sure `asyncapi-json-2` plugin was disabled
await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  }
});
```

**Removing** default parser plugin on **ad-hoc** basis:

```js
import { parse, options } from '@swagger-api/apidom-reference';

await parse('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
    parsers: options.parse.parsers.filter(parserPlugin => parserPlugin.name !== 'asyncapi-json-2'),
  }
});
```
As you can see, these are all primitive JavaScript Array manipulation techniques.
These techniques can be applied to replacing (use [Array.prototype.map()](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map)) or reordering parser plugins as well.


## Resolve component

`Resolve component` consists of two (2) sub-components: **File resolution** and **External Resolution**.
`Resolve component` is used by [Parse component](#parse-component) under the hood. `Resolve component` provides a resolved
file contents for a Parse component to parse.

### File resolution

Contains implementation of default [resolver plugins](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/resolvers).
Defaults resolver plugin is an object which knows how to obtain contents of a file represented by URI or URL.

#### Resolver plugins

File resolution comes with two (2) default resolver plugins.

##### [FileResolver](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/resolvers/file)

This resolver plugin is responsible for resolving a local file.
It detects if the provided URI represents a filesystem path and if so,
reads the file and provides its content.

**WARNING**: use this plugin with caution, as it can read files from a local file system.
By default, this plugin will reject to read any files from the local file system, unless
explicitly provided by **fileAllowList** option.

###### Providing file allow list

File allow list can be provided **globally** as an option to `FileResolver` in form
of array of *glob patterns* or *regular expressions*.

```js
import { options } from '@swagger-api/apidom-reference';
import FileResolver from '@swagger-api/apidom-reference/resolve/resolvers/file';
import HTTPResolverAxios from '@swagger-api/apidom-reference/resolve/resolvers/http-axios';

options.resolve.resolvers = [
  new FileResolver({
    fileAllowList: [
      '*.json',
      /\.json$/,
    ]
  }),
  new HTTPResolverAxios({ timeout: 5000, redirects: 5, withCredentials: false }),
]
```

File allow list can also be provided on ad-hoc basis:

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('/home/user/oas.json', {
  resolve: {
    resolverOpts: {
      fileAllowList: [
        '*.json',
        /\.json$/,
      ]
    },
  },
});
```

###### Caching results from remote files

The `cacheTTL` option controls caching behavior for files resolved from remote URLs or local filesystem paths. When
set to a value greater than `0`, resolved file contents are cached using
the [Cache Storage API](https://developer.mozilla.org/en-US/docs/Web/API/Cache) for the specified time-to-live (TTL)
duration in milliseconds.

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    resolverOpts: {
      cacheTTL: 60 * 1000, // store the result in a cache for 60 seconds
    },
  },
});
```

##### [HTTPResolverAxios](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/resolvers/http-axios)

This resolver plugin is responsible for resolving a remote file represented by HTTP(s) URL.
It detects if the provided URI represents an HTTP(s) URL and if so,
fetches the file and provides its content.

###### [Axios Request Config](https://axios-http.com/docs/req_config) support

HttpResolverAxios plugin supports all the options available in [Axios Request Config](https://axios-http.com/docs/req_config).
Config options can be provided in following way:

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    resolverOpts: {
      axiosConfig: {
        timeout: 10000,
        withCredentials: false,
        responseType: 'json',
      },
    },
  },
});
```

###### [Axios Interceptors](https://axios-http.com/docs/interceptors) support

HttpResolverAxios plugin supports [Axios Interceptors](https://axios-http.com/docs/interceptors).
Interceptors can be provided in following way:

```js
import { resolve } from '@swagger-api/apidom-reference';

const requestInterceptor = (config) => config;
const responseInterceptor = (response) => response;

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    resolverOpts: {
      axiosConfig: {
        interceptors: {
          request: requestInterceptor,
          response: responseInterceptor,
        },
      },
    },
  },
});
```

Multiple request and response interceptors can be provided in following way:

```js
import { resolve } from '@swagger-api/apidom-reference';

const requestInterceptor1 = (config) => config;
const requestInterceptor2 = (config) => config;
const responseInterceptor1 = (response) => response;
const responseInterceptor2 = async (error) => Promise.reject(error);

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    resolverOpts: {
      axiosConfig: {
        interceptors: {
          request: [requestInterceptor1, requestInterceptor2],
          response: [responseInterceptor1, responseInterceptor2],
        },
      },
    },
  },
});
```

**File resolution on local filesystem path**:

```js
import { readFile } from '@swagger-api/apidom-reference';

await readFile('/home/user/oas.json'); // Promise<Buffer>
```

**File resolution on HTTP(s) URL:**

```js
import { readFile } from '@swagger-api/apidom-reference';

await readFile('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json'); // Promise<Buffer>
```
File resolution always returns a [Promise](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise) containing a [Buffer](https://nodejs.org/api/buffer.html).
It is responsibility of the API consumer to transform `Buffer` into `String` or any other type.

```js
import { readFile } from '@swagger-api/apidom-reference';

const buffer = await readFile('/home/user/oas.json');
const string = buffer.toString('utf-8');
```

##### Resolver plugins execution order

It's important to understand that default resolver plugins are run in specific order. The order is determined
by the [options.resolve.resolvers](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/configuration/saturated.ts#L56) option.
Every plugin is pulled from `options.resolve.resolvers` option, and it's `canRead` method is called to determine
whether the plugin can resolve the URI. If `canRead` returns `true`, `read` method of plugin is called
and result from reading the file is returned. No subsequent resolver plugins are run.
If `canRead` returns `false`, next resolver plugin is pulled and this process is repeated until one
of the resolver plugins `canRead` method returns `true` or until entire list of resolver plugins is exhausted (throws error).

```js
[
  new FileResolver(),
  new HTTPResolverAxios({ timeout: 5000, redirects: 5, withCredentials: false }),
]
```

It's possible to **change** resolver plugins **order globally** by mutating global `resolve` option:

```js
import { options } from '@swagger-api/apidom-reference';
import FileResolver from '@swagger-api/apidom-reference/resolve/resolvers/file';
import HTTPResolverAxios from '@swagger-api/apidom-reference/resolve/resolvers/http-axios';

options.resolve.resolvers = [
  new HTTPResolverAxios({ timeout: 5000, redirects: 5, withCredentials: false }),
  new FileResolver(),
]
```

To **change** resolver plugins **order** on ad-hoc basis:

```js
import { readFile } from '@swagger-api/apidom-reference';
import FileResolver from '@swagger-api/apidom-reference/resolve/resolvers/file';
import HTTPResolverAxios from '@swagger-api/apidom-reference/resolve/resolvers/http-axios';

await readFile('/home/user/oas.json', {
  resolve: {
    resolvers: [
      new HTTPResolverAxios({ timeout: 5000, redirects: 5, withCredentials: false }),
      new FileResolver(),
    ],
  },
});
```

##### Resolver plugin options

Some resolver plugins accept additional options. It's possible to **change** resolver plugin
**options globally** by mutating global `resolve` options:

```js
import { options, readFile } from '@swagger-api/apidom-reference';

options.resolve.resolverOpts = {
  axiosConfig: {
    timeout: 10000,
  },
};

await readFile('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json');
```

To **change** the resolver plugins **options** on ad-hoc basis:

```js
import { readFile } from '@swagger-api/apidom-reference';

await readFile('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    resolverOpts: {
      axiosConfig: {
        timeout: 10000,
      },
    },
  },
});
```

Both of above examples will be using [HttpResolverAxios](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/resolvers/http-axios) plugin
(as we're trying to resolve HTTP(s) URL) and the `timeout` of resolution will increase from **default 3 seconds**
to 10 seconds.

##### Resolver strategy plugin options

Some resolver strategy plugins accept additional options. It's possible to **change** strategy plugin
**options globally** by mutating global `resolve` options:

```js
import { options, resolve } from '@swagger-api/apidom-reference';

options.resolve.strategyOpts = {
  apidom: { clone: true },
};

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json');
```

To **change** the resolver strategy plugins **options** on ad-hoc basis:

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  resolve: {
    strategyOpts: {
      apidom: { clone: true },
    },
  },
});
```

##### Creating new resolver plugin

Resolve component can be extended by additional resolver plugins. Every resolver plugin is an object that
must conform to the following interface/shape:

```typescript
interface ResolverPlugin {
  // uniquely identifies this plugin
  readonly name: string;

  // this method is called to determine whether the resolver plugin can resolve the file
  canRead(file: File, options: ReferenceOptions): boolean;
  // this method actually resolves the file
  read(file: File, options: ReferenceOptions): Promise<Buffer>;
}
```

New resolver plugin is then provided as an option to a `readFile` function:

```ts
import { readFile, options, File, Resolver, ResolverOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverOptions extends Omit<ResolverOptions, 'name'> {}

class MyCustomResolverPlugin extends Resolver {
  constructor(options?: MyCustomResolverOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver' });
  }

  async canRead(file: File, options: typeof options) {
    return true;
  }

  async read(file: File, options: typeof options) {
    // implementation of file resolution
  }
}

await readFile('/home/user/oas.json', {
  resolve: {
    resolvers: [...options.resolve.resolvers, new MyCustomResolverPlugin()],
  }
});
```

In this particular example we're adding our custom resolver plugin as the last plugin
to the available default resolver plugin list, so there's a good chance that one of the
default resolver plugins detects that it can resolve the `/home/user/oas.json` file,
resolves it and returns its content.

If you want to force execution of your custom plugin, add it as a first resolver plugin:

```ts
import { readFile, options, File, Resolver, ResolverOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverOptions extends Omit<ResolverOptions, 'name'> {}

class MyCustomResolverPlugin extends Resolver {
  constructor(options?: MyCustomResolverOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver' });
  }

  async canRead(file: File, options: typeof options) {
    return true;
  }

  async read(file: File, options: typeof options) {
    // implementation of file resolution
  }
}

await readFile('/home/user/oas.json', {
  resolve: {
    resolvers: [new MyCustomResolverPlugin(), ...options.resolve.resolvers],
  }
});
```

To override the default resolver plugins entirely, set `myCustomResolverPlugin` plugin to be the only one available:

```ts
import { readFile, options, File, Resolver, ResolverOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverOptions extends Omit<ResolverOptions, 'name'> {}

class MyCustomResolverPlugin extends Resolver {
  constructor(options?: MyCustomResolverOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver' });
  }

  async canRead(file: File, options: typeof options) {
    return true;
  }

  async read(file: File, options: typeof options) {
    // implementation of file resolution
  }
}

await readFile('/home/user/oas.json', {
  resolve: {
    resolvers: [new MyCustomResolverPlugin()],
  }
});
```
New resolver plugins can be based on two predefined abstract classes: [Resolver](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/resolvers/Resolver.ts) and [HTTPResolver](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/resolvers/HTTPResolver.ts).

##### Manipulating resolver plugins

Resolver plugins can be added, removed, replaced or reordered. We've already covered these techniques in [Manipulating parser plugins section](#manipulating-parser-plugins).

### External resolution

External resolution is a process of resolving all external dependencies of a particular
document using a specific [external resolution strategy](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies). External resolution strategy is determined by
asserting on `mediaType` option. [File Resolution](#file-resolution) (file content is read/fetched)
and [Parse component](#parse-component) (file content is parsed) are used under the hood.

**Externally resolving a file localed on a local filesystem:**

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('/home/user/oas.json', {
  parse: { mediType: 'application/vnd.oai.openapi+json;version=3.1.0' },
}); // Promise<ReferenceSet>
```

**Externally resolving an HTTP(S) URL located on an internet:**

```js
import { resolve } from '@swagger-api/apidom-reference';

await resolve('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  resolve: {
    resolverOpts: {
      axiosConfig: {
        timeout: 10
      },
    },
  },
}); // Promise<ReferenceSet>
```

**Externally resolving an ApiDOM fragment:**

When externally resolving an ApiDOM fragment, [baseURI](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/options/index.ts#L94)
resolve option needs to be provided to have a starting point for external dependency resolution.
`mediaType` parse option is unnecessary as we can directly assert the type of ApiDOM fragment.

```js
import { OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';
import { resolveApiDOM } from '@swagger-api/apidom-reference';

const apidom = OpenApi3_1Element.refract({
  openapi: '3.1.0',
  components: {
    parameters: {
      externalRef: {
        $ref: './ex.json#/externalParameter', // file is located at /home/user/ex.json
      }
    }
  }
});

const refSet = await resolveApiDOM(apidom, {
  resolve: { baseURI: '/home/user/' },
});

for (const ref of refSet) {
  console.log(ref.uri);
}
// /home/user
// /home/user/ex.json
```

[ReferenceSet](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/ReferenceSet.ts) is a [Set](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Set)
like structure containing list of [Reference](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/Reference.ts) objects.
Every Reference object represents single external dependency.

#### [External resolution strategies](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies)

External resolution strategy determines how a document is externally resolved. Depending on document `mediaType`
every strategy differs significantly. Resolve component comes with two (2) default external resolution strategies.

##### [apidom](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies/apidom)

External resolution strategy for understanding and resolving remote elements referenced with [Ref Element](https://apielements.org/en/latest/element-definitions.html?highlight=referencing#ref-element).

Supported media types:

```js
[
  'application/vnd.apidom',
  'application/vnd.apidom+json'
]
```

##### [asyncapi-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies/asyncapi-2)

External resolution strategy for understanding and resolving external dependencies of [AsyncApi 2.x.y](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md) definitions.

Supported media types:

```js
[
  'application/vnd.aai.asyncapi;version=2.0.0',
  'application/vnd.aai.asyncapi+json;version=2.0.0',
  'application/vnd.aai.asyncapi+yaml;version=2.0.0',
  'application/vnd.aai.asyncapi;version=2.1.0',
  'application/vnd.aai.asyncapi+json;version=2.1.0',
  'application/vnd.aai.asyncapi+yaml;version=2.1.0',
  'application/vnd.aai.asyncapi;version=2.2.0',
  'application/vnd.aai.asyncapi+json;version=2.2.0',
  'application/vnd.aai.asyncapi+yaml;version=2.2.0',
  'application/vnd.aai.asyncapi;version=2.3.0',
  'application/vnd.aai.asyncapi+json;version=2.3.0',
  'application/vnd.aai.asyncapi+yaml;version=2.3.0',
  'application/vnd.aai.asyncapi;version=2.4.0',
  'application/vnd.aai.asyncapi+json;version=2.4.0',
  'application/vnd.aai.asyncapi+yaml;version=2.4.0',
  'application/vnd.aai.asyncapi;version=2.5.0',
  'application/vnd.aai.asyncapi+json;version=2.5.0',
  'application/vnd.aai.asyncapi+yaml;version=2.5.0',
  'application/vnd.aai.asyncapi;version=2.6.0',
  'application/vnd.aai.asyncapi+json;version=2.6.0',
  'application/vnd.aai.asyncapi+yaml;version=2.6.0',
]
```

##### [openapi-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies/openapi-2)

External resolution strategy for understanding and resolving external dependencies of [OpenApi 2.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=2.0',
  'application/vnd.oai.openapi+json;version=2.0',
  'application/vnd.oai.openapi+yaml;version=2.0',
]
```

##### [openapi-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies/openapi-3-0)

External resolution strategy for understanding and resolving external dependencies of [OpenApi 3.0.x](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=3.0.0',
  'application/vnd.oai.openapi+json;version=3.0.0',
  'application/vnd.oai.openapi+yaml;version=3.0.0',
  'application/vnd.oai.openapi;version=3.0.1',
  'application/vnd.oai.openapi+json;version=3.0.1',
  'application/vnd.oai.openapi+yaml;version=3.0.1',
  'application/vnd.oai.openapi;version=3.0.2',
  'application/vnd.oai.openapi+json;version=3.0.2',
  'application/vnd.oai.openapi+yaml;version=3.0.2',
  'application/vnd.oai.openapi;version=3.0.3',
  'application/vnd.oai.openapi+json;version=3.0.3',
  'application/vnd.oai.openapi+yaml;version=3.0.3',
  'application/vnd.oai.openapi;version=3.0.4',
  'application/vnd.oai.openapi+json;version=3.0.4',
  'application/vnd.oai.openapi+yaml;version=3.0.4',
]
```

##### [openapi-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/resolve/strategies/openapi-3-1)

External resolution strategy for understanding and resolving external dependencies of [OpenApi 3.1.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=3.1.0',
  'application/vnd.oai.openapi+json;version=3.1.0',
  'application/vnd.oai.openapi+yaml;version=3.1.0'
]
```

##### External resolution strategies execution order

It's important to understand that default external resolution strategies are run in specific order. The order is determined
by the [options.resolve.strategies](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/configuration/saturated.ts#L61) option.
Every strategy is pulled from `options.resolve.strategies` option and its `canResolve` method is called to determine
whether the strategy can externally resolve the URI. If `canResolve` returns `true`, `resolve` method of strategy is called
and result from external resolution is returned. No subsequent strategies  are run. If `canResolve` returns
`false`, next strategy is pulled and this process is repeated until one of the strategy's `canResolve` method
returns `true` or until entire list of strategies is exhausted (throws error).

```js
[
  new OpenAPI2ResolveStrategy(),
  new OpenAPI3_0ResolveStrategy(),
  new OpenAPI3_1ResolveStrategy(),
  new AsyncAPI2ResolveStrategy(),
  new ApiDOMResolveStrategy(),
]
```
Most specific strategies are listed first, most generic are listed last.

It's possible to **change** strategies **order globally** by mutating global `resolve` option:

```js
import { options } from '@swagger-api/apidom-reference';
import ApiDOMResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/apidom';
import AsyncAPI2ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/asyncapi-2';
import OpenAPI2ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-2';
import OpenAPI3_0ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-3-0';
import OpenAPI3_1ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-3-1';

options.resolve.strategies = [
  new OpenAPI2ResolveStrategy(),
  new OpenAPI3_0ResolveStrategy(),
  new OpenAPI3_1ResolveStrategy(),
  new AsyncAPI2ResolveStrategy(),
  new ApiDOMResolveStrategy(),
];
```

To **change** the strategies **order** on ad-hoc basis:

```js
import { resolve } from '@swagger-api/apidom-reference';
import ApiDOMResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/apidom';
import AsyncAPI2ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/asyncapi-2';
import OpenAPI2ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-2';
import OpenAPI3_0ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-3-0';
import OpenAPI3_1ResolveStrategy from '@swagger-api/apidom-reference/resolve/strategies/openapi-3-1';


await resolve('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  resolve: {
    strategies: [
      new AsyncAPI2ResolveStrategy(),
      new OpenAPI2ResolveStrategy(),
      new OpenAPI3_0ResolveStrategy(),
      new OpenAPI3_1ResolveStrategy(),
      new ApiDOMResolveStrategy(),
    ]
  }
});
```
##### Creating new external resolution strategy

Resolve component can be extended by additional strategies. Every strategy is an object that
must conform to the following interface/shape:

```typescript
interface ResolveStrategy {
  // uniquely identifies this plugin
  readonly name: string;

  // this method is called to determine whether the strategy can externally resolve the file
  canResolve(file: File, options: ReferenceOptions): boolean;
  // this method actually externally resolves the file
  resolve(file: File, options: ReferenceOptions): Promise<ReferenceSet>;
}
```

New strategy is then provided as an option to a `resolve` function:

```ts
import { resolve, options, File, ResolveStrategy, ResolveStrategyOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverStrategyOptions extends Omit<ResolveStrategyOptions, 'name'> {}

class MyCustomResolverStrategy extends ResolveStrategy {
  constructor(options?: MyCustomResolverStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver-strategy' });
  }

  canResolve(file: File, options: typeof options) {
    return true;
  }

  async resolve(file: File, options: typeof options) {
    // implementation of external resolution
  }
}

await resolve('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  resolve: {
    strategies: [...options.resolve.strategies, new MyCustomResolverStrategy()],
  }
});
```

In this particular example we're adding our custom strategy as the last strategy
to the available default external resolution strategy list, so there's a good chance that one of the
default strategies detects that it can externally resolve the `/home/user/oas.json` file,
resolves it and returns `ReferenceSet` object.

If you want to force execution of your strategy, add it as a first one:

```ts
import { resolve, options, File, ResolveStrategy, ResolveStrategyOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverStrategyOptions extends Omit<ResolveStrategyOptions, 'name'> {}

class MyCustomResolverStrategy extends ResolveStrategy {
  constructor(options?: MyCustomResolverStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver-strategy' });
  }

  canResolve(file: File, options: typeof options) {
    return true;
  }

  async resolve(file: File, options: typeof options) {
    // implementation of external resolution
  }
}

await resolve('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  resolve: {
    strategies: [new MyCustomResolverStrategy(), ...options.resolve.strategies],
  }
});
```

To override the default strategies entirely, set `MyCustomResolverStrategy` strategy to be the only one available:

```ts
import { resolve, options, File, ResolveStrategy, ResolveStrategyOptions } from '@swagger-api/apidom-reference';

interface MyCustomResolverStrategyOptions extends Omit<ResolveStrategyOptions, 'name'> {}

class MyCustomResolverStrategy extends ResolveStrategy {
  constructor(options?: MyCustomResolverStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-resolver-strategy' });
  }

  canResolve(file: File, options: typeof options) {
    return true;
  }

  async resolve(file: File, options: typeof options) {
    // implementation of external resolution
  }
}

await resolve('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  resolve: {
    strategies: [new MyCustomResolverPlugin()],
  }
});
```
New strategies can be based on a predefined abstract class called [ResolveStrategy](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/resolve/strategies/ResolveStrategy.ts).

##### Manipulating external resolution strategies

External resolution strategies can be added, removed, replaced or reordered. We've already covered these techniques in [Manipulating parser plugins section](#manipulating-parser-plugins).

## Dereference component

Dereferencing is a process of transcluding referencing element (internal or external) with a referenced element
using a specific [dereference strategy](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies). Simply put, dereferencing is a process of reference removal.
Dereferencing strategy is determined by asserting on `mediaType` option. [File Resolution](#file-resolution) (file content is read/fetched)
and [Parse component](#parse-component) (file content is parsed) are used under the hood.

**Dereferencing a file localed on a local filesystem:**

```js
import { dereference } from '@swagger-api/apidom-reference';

await dereference('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
}); // Promise<ParseResultElement>
```

**Dereferencing an HTTP(S) URL located on an internet:**

```js
import { dereference } from '@swagger-api/apidom-reference';

await dereference('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  resolve: {
    resolverOpts: {
      axiosConfig: {
        timeout: 10
      },
    },
  },
}); // Promise<ParseResultElement>
```

**Dereferencing an ApiDOM fragment:**

When dereferencing an ApiDOM fragment, [baseURI](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/options/index.ts#L94)
resolve option needs to be provided to have a starting point for external dependency resolution.
`mediaType` parse option is unnecessary as we can directly assert the type of ApiDOM fragment.

**ex.json**

```json
{
  "externalParameter": {
    "name": "param1",
    "in": "query"
  }
}
```

```js
import { OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';
import { dereferenceApiDOM } from '@swagger-api/apidom-reference';

const apidom = OpenApi3_1Element.refract({
  openapi: '3.1.0',
  components: {
    parameters: {
      externalRef: {
        $ref: './ex.json#/externalParameter', // file is located at /home/user/ex.json
      }
    }
  }
});

const dereferenced = await dereferenceApiDOM(apidom, {
  resolve: { baseURI: '/home/user/' },
});
/**
 * OpenApi3_1Element {
 *   openapi: '3.1.0',
 *   components: {
 *     parameters: {
 *       externalRef: {
 *         name: param1,
 *         in: query
 *       }
 *     }
 *   }
 * }
 */
```

#### [Dereference strategies](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies)

Dereference strategy determines how a document is internally or externally dereferenced. Depending on document `mediaType` option,
every strategy differs significantly. `Dereference component` comes with four (4) default dereference strategies.

#### [apidom](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies/apidom)

Dereference strategy for dereferencing ApiDOM using [Ref Element](https://apielements.org/en/latest/element-definitions.html?highlight=referencing#ref-element).
Ref Element MAY be used to reference elements in remote documents or elements in the local document.
The ref element transcludes the contents of the element into the document in which it is referenced.

Supported media types:

```js
[
  'application/vnd.apidom',
  'application/vnd.apidom+json',
]
```

##### [asyncapi-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies/asyncapi-2)

Dereference strategy for dereferencing [AsyncApi 2.x.y](https://github.com/asyncapi/spec/blob/master/spec/asyncapi.md) definitions.

Supported media types:

```js
[
  'application/vnd.aai.asyncapi;version=2.0.0',
  'application/vnd.aai.asyncapi+json;version=2.0.0',
  'application/vnd.aai.asyncapi+yaml;version=2.0.0',
  'application/vnd.aai.asyncapi;version=2.1.0',
  'application/vnd.aai.asyncapi+json;version=2.1.0',
  'application/vnd.aai.asyncapi+yaml;version=2.1.0',
  'application/vnd.aai.asyncapi;version=2.2.0',
  'application/vnd.aai.asyncapi+json;version=2.2.0',
  'application/vnd.aai.asyncapi+yaml;version=2.2.0',
  'application/vnd.aai.asyncapi;version=2.3.0',
  'application/vnd.aai.asyncapi+json;version=2.3.0',
  'application/vnd.aai.asyncapi+yaml;version=2.3.0',
  'application/vnd.aai.asyncapi;version=2.4.0',
  'application/vnd.aai.asyncapi+json;version=2.4.0',
  'application/vnd.aai.asyncapi+yaml;version=2.4.0',
  'application/vnd.aai.asyncapi;version=2.5.0',
  'application/vnd.aai.asyncapi+json;version=2.5.0',
  'application/vnd.aai.asyncapi+yaml;version=2.5.0',
  'application/vnd.aai.asyncapi;version=2.6.0',
  'application/vnd.aai.asyncapi+json;version=2.6.0',
  'application/vnd.aai.asyncapi+yaml;version=2.6.0',
]
```

##### [openapi-2](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies/openapi-2)

Dereference strategy for dereferencing [OpenApi 2.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=2.0',
  'application/vnd.oai.openapi+json;version=2.0',
  'application/vnd.oai.openapi+yaml;version=2.0',
]
```

##### [openapi-3-0](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies/openapi-3-0)

Dereference strategy for dereferencing [OpenApi 3.0.x](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=3.0.0',
  'application/vnd.oai.openapi+json;version=3.0.0',
  'application/vnd.oai.openapi+yaml;version=3.0.0',
  'application/vnd.oai.openapi;version=3.0.1',
  'application/vnd.oai.openapi+json;version=3.0.1',
  'application/vnd.oai.openapi+yaml;version=3.0.1',
  'application/vnd.oai.openapi;version=3.0.2',
  'application/vnd.oai.openapi+json;version=3.0.2',
  'application/vnd.oai.openapi+yaml;version=3.0.2',
  'application/vnd.oai.openapi;version=3.0.3',
  'application/vnd.oai.openapi+json;version=3.0.3',
  'application/vnd.oai.openapi+yaml;version=3.0.3',
  'application/vnd.oai.openapi;version=3.0.4',
  'application/vnd.oai.openapi+json;version=3.0.4',
  'application/vnd.oai.openapi+yaml;version=3.0.4',
]
```

##### [openapi-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/dereference/strategies/openapi-3-1)

Dereference strategy for dereferencing [OpenApi 3.1.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=3.1.0',
  'application/vnd.oai.openapi+json;version=3.1.0',
  'application/vnd.oai.openapi+yaml;version=3.1.0'
]
```

##### Dereference strategies execution order

It's important to understand that default dereference strategies are run in specific order. The order is determined
by the [options.dereference.strategies](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/configuration/saturated.ts#L69) option.
Every strategy is pulled from `options.dereference.strategies` option and it's `canDereference` method is called to determine
whether the strategy can dereference the URI. If `canDereference` returns `true`, `dereference` method of strategy is called
and result from dereferencing is returned. No subsequent strategies  are run. If `canDereference` returns
`false`, next strategy is pulled and this process is repeated until one of the strategy's `canDereference` method
returns `true` or until entire list of strategies is exhausted (throws error).

```js
[
  new OpenAPI2DereferenceStrategy(),
  new OpenAPI3_0DereferenceStrategy(),
  new OpenAPI3_1DereferenceStrategy(),
  new AsyncAPI2DereferenceStrategy(),
  new AsyncAPI3DereferenceStrategy(),
  new ApiDOMDereferenceStrategy(),
]
```
Most specific strategies are listed first, most generic are listed last.

It's possible to **change** strategies **order globally** by mutating global `dereference` option:

```js
import { options } from '@swagger-api/apidom-reference';
import AsyncAPI2DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/asyncapi-2';
import AsyncAPI3DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/asyncapi-3';
import OpenAPI2DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-2';
import OpenAPI3_0DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-3-0';
import OpenAPI3_1DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-3-1';
import ApiDOMDereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/apidom';

options.dereference.strategies = [
  new OpenAPI2DereferenceStrategy(),
  new OpenAPI3_0DereferenceStrategy(),
  new OpenAPI3_1DereferenceStrategy(),
  new AsyncAPI2DereferenceStrategy(),
  new AsyncAPI3DereferenceStrategy(),
  new ApiDOMDereferenceStrategy(),
];
```

To **change** the strategies **order** on ad-hoc basis:

```js
import { dereference } from '@swagger-api/apidom-reference';
import AsyncAPI2DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/asyncapi-2';
import AsyncAPI3DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/asyncapi-3';
import OpenAPI2DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-2';
import OpenAPI3_0DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-3-0';
import OpenAPI3_1DereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/openapi-3-1';
import ApiDOMDereferenceStrategy from '@swagger-api/apidom-reference/dereference/strategies/apidom';


await dereference('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  dereference: {
    strategies: [
      new AsyncAPI2DereferenceStrategy(),
      new AsyncAPI3DereferenceStrategy(),
      new OpenAPI2DereferenceStrategy(),
      new OpenAPI3_0DereferenceStrategy(),
      new OpenAPI3_1DereferenceStrategy(),
      new ApiDOMDereferenceStrategy(),
    ]
  }
});
```

##### Dereference strategy plugin options

Some dereference strategy plugins accept additional options. It's possible to **change** strategy plugin
**options globally** by mutating global `dereference` options:

```js
import { options, dereference } from '@swagger-api/apidom-reference';

options.dereference.strategyOpts = {
  apidom: { clone: true },
};

await dereference('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json');
```

To **change** the dereference strategy plugins **options** on ad-hoc basis:

```js
import { dereference } from '@swagger-api/apidom-reference';

await dereference('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  dereference: {
    strategyOpts: {
      apidom: { clone: true },
    },
  },
});
```

##### Creating new dereference strategy

Dereference component can be extended by additional strategies. Every strategy is an object that
must conform to the following interface/shape:

```ts
interface DereferenceStrategy {
  // uniquely identifies this plugin
  readonly name: string,

  // this method is called to determine whether the strategy can dereference the file
  canDereference(file: File, options: ReferenceOptions): boolean;
  // this method actually dereferences the file
  dereference(file: File, options: ReferenceOptions): Promise<Element>;
}
```

New strategy is then provided as an option to the `dereference` function:

```ts
import { dereference, options, File, DereferenceStrategy, DereferenceStrategyOptions } from '@swagger-api/apidom-reference';

export interface MyCustomDereferenceStrategyOptions
  extends Omit<DereferenceStrategyOptions, 'name'> {}

class MyCustomDereferenceStrategy extends DereferenceStrategy {
  constructor(options?: MyCustomDereferenceStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-dereference' });
  }

  canDereference(file: File, options: typeof options) {
    return true;
  }

  async dereference(file: File, options: typeof options) {
     // implementation of dereferenceing
  }
}

await dereference('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  dereference: {
    strategies: [...options.dereference.strategies, new MyCustomDereferenceStrategy()],
  }
});
```

In this particular example we're adding our custom strategy as the last strategy
to the available default dereference strategy list, so there's a good chance that one of the
default strategies detects that it can dereference the `/home/user/oas.json` file,
dereferences it and returns a dereferenced element.

If you want to force execution of your strategy, add it as a first one:

```ts
import { dereference, options, File, DereferenceStrategy, DereferenceStrategyOptions } from '@swagger-api/apidom-reference';

export interface MyCustomDereferenceStrategyOptions
extends Omit<DereferenceStrategyOptions, 'name'> {}

class MyCustomDereferenceStrategy extends DereferenceStrategy {
  constructor(options?: MyCustomDereferenceStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-dereference' });
  }

  canDereference(file: File, options: typeof options) {
    return true;
  }

  async dereference(file: File, options: typeof options) {
    // implementation of dereferenceing
  }
}

await dereference('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  dereference: {
    strategies: [new MyCustomDereferenceStrategy(), ...options.dereference.strategies],
  }
});
```

To override the default strategies entirely, set `MyCustomDereferenceStrategy` strategy to be the only one available:

```ts
import { dereference, options, File, DereferenceStrategy, DereferenceStrategyOptions } from '@swagger-api/apidom-reference';

export interface MyCustomDereferenceStrategyOptions
extends Omit<DereferenceStrategyOptions, 'name'> {}

class MyCustomDereferenceStrategy extends DereferenceStrategy {
  constructor(options?: MyCustomDereferenceStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-dereference' });
  }

  canDereference(file: File, options: typeof options) {
    return true;
  }

  async dereference(file: File, options: typeof options) {
    // implementation of dereferenceing
  }
}


await dereference('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  dereference: {
    strategies: [new MyCustomDereferenceStrategy()],
  }
});
```

New strategies can be based on a predefined abstract class called [DereferenceStrategy](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/dereference/strategies/DereferenceStrategy.ts).

##### Manipulating dereference strategies

Dereference strategies can be added, removed, replaced or reordered. We've already covered these techniques in [Manipulating parser plugins section](#manipulating-parser-plugins).

##### Increasing speed of dereference

Our default dereference strategies are built on asynchronous sequential traversing of ApiDOM.
The total time of dereferencing is the sum of `traversing` + sum of `external resolution per file`.
By having a huge number of external dependencies in your definition file, dereferencing can get quite slow.
Fortunately there is solution for this by running an `external resolution` first,
and passing its result to dereferencing via an option. External resolution ignores internal references,
so it's theoretically always faster than the dereferencing.

```js
import { resolve, dereference } from '@swagger-api/apidom-reference';

const refSet = await resolve('/home/user/oas.json', {
  parse: { mediType: 'application/vnd.oai.openapi+json;version=3.1.0' },
});

const dereferenced = await dereference('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  dereference: { refSet },
});
```

## Bundle component

Bundling is a convenient way to package up resources spread across multiple files in a single file
(**Compound Document**) using a specific [bundle strategy](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/bundle/strategies).

The bundling process for creating a Compound Document is defined as taking references (such as "$ref")
to an external Resource and embedding the referenced Resources within the referring document.
Bundling SHOULD be done in such a way that all URIs (used for referencing) in the base document
and any referenced/embedded documents do not require altering.

Bundling strategy is determined by asserting on `mediaType` option. [File Resolution](#file-resolution) (file content is read/fetched)
and [Parse component](#parse-component) (file content is parsed) are used under the hood.

**Bundling a file localed on a local filesystem:**

```js
import { bundle } from '@swagger-api/apidom-reference';

await bundle('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
}); // Promise<ParseResultElement>
```

**Bundling an HTTP(S) URL located on an internet:**

```js
import { bundle } from '@swagger-api/apidom-reference';

await bundle('https://raw.githubusercontent.com/OAI/OpenAPI-Specification/main/examples/v3.1/webhook-example.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  resolve: {
    resolverOpts: {
      axiosConfig: {
        timeout: 10
      },
    },
  },
}); // Promise<ParseResultElement>
```

#### [Bundle strategies](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/bundle/strategies)

Bundle strategy determines how a document is bundled into a Compound Document. Depending on document `mediaType` option,
every strategy differs significantly. `Bundle component` comes with single (1) default bundle strategy.

##### [openapi-3-1](https://github.com/swagger-api/apidom/tree/main/packages/apidom-reference/src/bundle/strategies/openapi-3-1)

Bundle strategy for bundling [OpenApi 3.1.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md) definitions.

Supported media types:

```js
[
  'application/vnd.oai.openapi;version=3.1.0',
  'application/vnd.oai.openapi+json;version=3.1.0',
  'application/vnd.oai.openapi+yaml;version=3.1.0'
]
```

##### Bundle strategies execution order

It's important to understand that default bundle strategies are run in specific order. The order is determined
by the [options.bundle.strategies](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/configuration/saturated.ts#L78) option.
Every strategy is pulled from `options.bundle.strategies` option, and it's `canBundle` method is called to determine
whether the strategy can bundle the URI. If `canBundle` returns `true`, `bundle` method of strategy is called
and result from bundling is returned. No subsequent strategies are run. If `canBundle` returns
`false`, next strategy is pulled and this process is repeated until one of the strategy's `canBundle` method
returns `true` or until entire list of strategies is exhausted (throws error).

```js
[
  new OpenAPI3_1BundleStrategy(),
]
```
Most specific strategies are listed first, most generic are listed last.

It's possible to **change** strategies **order globally** by mutating global `bundle` option:

```js
import { options } from '@swagger-api/apidom-reference';
import OpenAPI3_1BundleStrategy from '@swagger-api/apidom-reference/bundle/strategies/openapi-3-1'

options.dereference.strategies = [
  new OpenAPI3_1DereferenceStrategy(),
];
```

To **change** the strategies **order** on ad-hoc basis:

```js
import { bundle } from '@swagger-api/apidom-reference';
import OpenAPI3_1BundleStrategy from '@swagger-api/apidom-reference/bundle/strategies/openapi-3-1'

await bundle('/home/user/oas.json', {
  parse: {
    mediaType: 'application/vnd.oai.openapi+json;version=3.1.0',
  },
  bundle: {
    strategies: [
      new OpenAPI3_1BundleStrategy(),
    ]
  }
});
```
##### Creating new bundle strategy

Bundle component can be extended by additional strategies. Every strategy is an object that
must conform to the following interface/shape:

```typescript
interface BundleStrategy {
  // uniquely identifies this plugin
  readonly name: string;

  // this method is called to determine whether the strategy can bundle the file
  canBundle(file: File, options: ReferenceOptions): boolean;
  // this method actually bundles the file
  bundle(file: File, options: ReferenceOptions): Promise<ParseResultElement>;
}
```

New strategy is then provided as an option to the `bundle` function:

```ts
import { bundle, options, File, BundleStrategy } from '@swagger-api/apidom-reference';

interface MyCustomBundleStrategyOptions extends Omit<BundleStrategyOptions, 'name'> {}

class MyCustomBundleStrategy extends BundleStrategy {
  constructor(options?: MyCustomBundleStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-bundle-strategy' });
  }

  canBundle(file: File, options: typeof options) {
    return true;
  }
  async bundle(file: File, options: typeof options) {
     // implementation of bundling
  }
}

await bundle('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  bundle: {
    strategies: [...options.bundle.strategies, new MyCustomBundleStrategy()],
  }
});
```

In this particular example we're adding our custom strategy as the last strategy
to the available default bundle strategy list, so there's a good chance that one of the
default strategies detects that it can bundle the `/home/user/oas.json` file,
bundles it and returns a bundled element.

If you want to force execution of your strategy, add it as a first one:

```ts
import { bundle, options, File, BundleStrategy } from '@swagger-api/apidom-reference';

interface MyCustomBundleStrategyOptions extends Omit<BundleStrategyOptions, 'name'> {}

class MyCustomBundleStrategy extends BundleStrategy {
  constructor(options?: MyCustomBundleStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-bundle-strategy' });
  }

  canBundle(file: File, options: typeof options) {
    return true;
  }
  async bundle(file: File, options: typeof options) {
    // implementation of bundling
  }
}

await bundle('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  bundle: {
    strategies: [new MyCustomBundleStrategy(), ...options.bundle.strategies],
  }
});
```

To override the default strategies entirely, set `MyCustomBundleStrategy` strategy to be the only one available:

```ts
import { bundle, options, File, BundleStrategy } from '@swagger-api/apidom-reference';

interface MyCustomBundleStrategyOptions extends Omit<BundleStrategyOptions, 'name'> {}

class MyCustomBundleStrategy extends BundleStrategy {
  constructor(options?: MyCustomBundleStrategyOptions) {
    super({ ...(options ?? {}), name: 'my-custom-bundle-strategy' });
  }

  canBundle(file: File, options: typeof options) {
    return true;
  }
  async bundle(file: File, options: typeof options) {
    // implementation of bundling
  }
}

await bundle('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  bundle: {
    strategies: [new MyCustomBundleStrategy()],
  }
});
```

New strategies can be based on a predefined abstract class called [BundleStrategy](https://github.com/swagger-api/apidom/blob/main/packages/apidom-reference/src/bundle/strategies/BundleStrategy.ts).

##### Manipulating bundle strategies

Bundle strategies can be added, removed, replaced or reordered. We've already covered these techniques in [Manipulating parser plugins section](#manipulating-parser-plugins).

##### Increasing speed of bundling

Our default bundling strategies are built on asynchronous sequential traversing of ApiDOM.
The total time of bundling is the sum of `traversing` + sum of `external resolution per referencing element`.
By having a huge number of external dependencies in your definition file, bundling can get quite slow.
Fortunately there is solution for this by running an `external resolution` first,
and passing its result to bundling via an option. External resolution is built on asynchronous parallel traversal (on single file),
so it's theoretically always faster on huge amount of external dependencies than the bundling.

```js
import { resolve, bundle } from '@swagger-api/apidom-reference';

const refSet = await resolve('/home/user/oas.json', {
  parse: { mediType: 'application/vnd.oai.openapi+json;version=3.1.0' },
});

const bundled = await bundle('/home/user/oas.json', {
  parse: { mediaType: 'application/vnd.oai.openapi+json;version=3.1.0' },
  bundle: { refSet },
});
```

Total time of bundling is now the sum of `external resolution traversing` + `bundle traversing` + sum of `max external resolution per file`.
