# @swagger-api/apidom-ns-openapi-3-0

`@swagger-api/apidom-ns-openapi-3-0` contains ApiDOM namespace supports following OpenAPI specification versions:

- [OpenAPI 3.0.4 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md)
- [OpenAPI 3.0.3 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.3.md)
- [OpenAPI 3.0.2 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.2.md)
- [OpenAPI 3.0.1 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.1.md)
- [OpenAPI 3.0.0 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.0.md)

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-openapi-3-0
```

## OpenAPI 3.0.x namespace

OpenAPI 3.0.x namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-3-0/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import openApi3_0Namespace from '@swagger-api/apidom-ns-openapi-3-0';

const namespace = createNamespace(openApi3_0Namespace);

const objectElement = new namespace.elements.Object();
const openApiElement = new namespace.elements.OpenApi3_0();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { OpenApi3_0Element, InfoElement } from '@swagger-api/apidom-ns-openapi-3-0';

const infoElement = new InfoElement();
const openApiElement = new OpenApi3_0Element();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-0/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isOpenApi3_0Element, OpenApi3_0Element } from '@swagger-api/apidom-ns-openapi-3-0';

const openApiElement = new OpenApi3_0Element();

isOpenApi3_0Element(openApiElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-0/src/traversal/visitor.ts#L24) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-0/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { OpenApi3_0Element, keyMap, getNodeType } from '@swagger-api/apidom-ns-openapi-3-0';

const element = new OpenApi3_0Element();

const visitor = {
  OpenApi3_0Element(openApiElement) {
    console.dir(openApiElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-0';

const object = {
    title: 'my title',
    description: 'my description',
    version: '0.1.0',
};

InfoElement.refract(object); // => InfoElement({ title, description, version })
```

**Refracting generic ApiDOM structures**:

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-0';

const objectElement = new ObjectElement({
    title: 'my title',
    description: 'my description',
    version: '0.1.0',
});

InfoElement.refract(objectElement); // => InfoElement({ title = 'my title', description = 'my description', version = '0.1.0' })
```

### Refractor plugins

Refractors can accept plugins as a second argument of refract static method.

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-0';

const objectElement = new ObjectElement({
    title: 'my title',
    description: 'my description',
    version: '0.1.0',
});

const plugin = ({ predicates, namespace }) => ({
  name: 'plugin',
  pre() {
      console.dir('runs before traversal');
  },
  visitor: {
    InfoElement(infoElement) {
      infoElement.version = '2.0.0';
    },
  },
  post() {
      console.dir('runs after traversal');
  },
});

InfoElement.refract(objectElement, { plugins: [plugin] }); // => InfoElement({ title = 'my title', description = 'my description', version = '2.0.0' })
```

You can define as many plugins as needed to enhance the resulting namespaced ApiDOM structure.
If multiple plugins with the same visitor method are defined, they run in parallel (just like in Babel).

#### Replace Empty Element plugin

This plugin is specific to YAML 1.2 format, which allows defining key-value pairs with empty key,
empty value, or both. If the value is not provided in YAML format, this plugin compensates for
this missing value with the most appropriate semantic element type.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginReplaceEmptyElement, OpenApi3_0Element } from '@swagger-api/apidom-ns-openapi-3-0';

const yamlDefinition = `
openapi: 3.0.4
info:
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_0Element.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (OpenApi3_0Element
//   (MemberElement
//     (StringElement)
//     (OpenapiElement))
//   (MemberElement
//     (StringElement)
//     (InfoElement)))

// => without the plugin the result would be as follows:
// (OpenApi3_0Element
//   (MemberElement
//     (StringElement)
//     (OpenapiElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [OpenAPI Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#openapi-object)
- [x] [Info Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#info-object)
- [x] [Contact Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#contact-object)
- [x] [License Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#license-object)
- [x] [Server Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#server-object)
- [x] [Server Variable Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#server-variable-object)
- [x] [Components](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#components-object)
- [x] [Paths Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#paths-object)
- [x] [Path Item Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#path-item-object)
- [x] [Operation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#operation-object)
- [x] [External Documentation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#external-documentation-object)
- [x] [Parameter Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#parameter-object)
- [x] [Request Body Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#request-body-object)
- [x] [Media Type Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#media-type-object)
- [x] [Encoding Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#encoding-object)
- [x] [Responses Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#responses-object)
- [x] [Callback Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#callback-object)
- [x] [Example Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#example-object)
- [x] [Link Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#link-object)
- [x] [Header Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#header-object)
- [x] [Tag Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#tag-object)
- [x] [Reference Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#reference-object)
- [x] [Schema Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#schema-object)
- [x] [Discriminator Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#discriminator-object)
- [x] [XML Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#xml-object)
- [x] [Security Scheme Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#security-scheme-object)
- [x] [OAuth Flows Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#oauth-flows-object)
- [x] [OAuth Flow Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#oauth-flow-object)
- [x] [Security Requirement Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#security-requirement-object)
- [x] [Specification extensions](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#specification-extensions)

