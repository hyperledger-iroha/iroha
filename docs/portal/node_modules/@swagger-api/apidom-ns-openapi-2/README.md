# @swagger-api/apidom-ns-openapi-2

`@swagger-api/apidom-ns-openapi-2` contains ApiDOM namespace supports following OpenAPI specification versions:

- [OpenAPI 2.0 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md)

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-openapi-2
```

## OpenAPI 2.0 namespace

OpenAPI 2.0 namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-2/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import openApi2Namespace from '@swagger-api/apidom-ns-openapi-2';

const namespace = createNamespace(openApi2Namespace);

const objectElement = new namespace.elements.Object();
const swaggerElement = new namespace.elements.Swagger();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { SwaggerElement, InfoElement } from '@swagger-api/apidom-ns-openapi-2';

const infoElement = new InfoElement();
const swaggerElement = new SwaggerElement();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-2/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isSwaggerElement, SwaggerElement } from '@swagger-api/apidom-ns-openapi-2';

const swaggerElement = new SwaggerElement();

isSwaggerElement(swaggerElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-2/src/traversal/visitor.ts#L24) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-2/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { SwaggerElement, keyMap, getNodeType } from '@swagger-api/apidom-ns-openapi-2';

const element = new SwaggerElement();

const visitor = {
  SwaggerElement(swaggerElement) {
    console.dir(swaggerElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { InfoElement } from '@swagger-api/apidom-ns-openapi-2';

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
import { InfoElement } from '@swagger-api/apidom-ns-openapi-2';

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
import { InfoElement } from '@swagger-api/apidom-ns-openapi-2';

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
import { refractorPluginReplaceEmptyElement, SwaggerElement } from '@swagger-api/apidom-ns-openapi-2';

const yamlDefinition = `
swagger: 2.0
info:
`;
const apiDOM = await parse(yamlDefinition);
const swaggerElement = SwaggerElement.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (SwaggerElement
//   (MemberElement
//     (StringElement)
//     (SwaggerVersionElement))
//   (MemberElement
//     (StringElement)
//     (InfoElement)))

// => without the plugin the result would be as follows:
// (SwaggerElement
//   (MemberElement
//     (StringElement)
//     (SwaggerVersionElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [Swagger Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-swagger-object)
- [x] [Info Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-info-object)
- [x] [Contact Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-contact-object)
- [x] [License Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-license-object)
- [x] [Paths Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-paths-object)
- [x] [Path Item Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-path-item-object)
- [x] [Operation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-operation-object)
- [x] [External Documentation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-external-documentation-object)
- [x] [Parameter Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-parameter-object)
- [x] [Items Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-items-object)
- [x] [Responses Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-responses-object)
- [x] [Response Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-response-object)
- [x] [Headers Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-headers-object)
- [x] [Example Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-example-object)
- [x] [Header Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-header-object)
- [x] [Tag Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-tag-object)
- [x] [Reference Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-reference-object)
- [x] [JSON Reference Object](https://datatracker.ietf.org/doc/html/draft-pbryan-zyp-json-ref-02)
- [x] [Schema Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-schema-object)
- [x] [XML Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-xml-object)
- [x] [Definitions Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-definitions-object)
- [x] [Parameters Definitions Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-parameters-definitions-object)
- [x] [Responses Definitions Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-responses-definitions-object)
- [x] [Security Definitions Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-security-definitions-object)
- [x] [Security Scheme Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-security-scheme-object)
- [x] [Scopes Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-scopes-object)
- [x] [Security Requirement Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-security-requirement-object)
- [x] [Specification extensions](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#user-content-specification-extensions)

