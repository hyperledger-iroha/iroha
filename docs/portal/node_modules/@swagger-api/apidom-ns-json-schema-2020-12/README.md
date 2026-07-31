# @swagger-api/apidom-ns-json-schema-2020-12

`@swagger-api/apidom-ns-json-schema-2020-12` contains ApiDOM namespace specific to [JSON Schema 2020-12](https://json-schema.org/draft/2020-12/draft-bhutton-json-schema-01) specification.

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-json-schema-2020-12
```

## JSON Schema 2020-12 namespace

JSON Schema 2020-12 namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-json-schema-2020-12/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import jsonShema202012Namespace from '@swagger-api/apidom-ns-json-schema-2020-12';

const namespace = createNamespace(jsonShema202012Namespace);

const objectElement = new namespace.elements.Object();
const jsonSchemaElement = new namespace.elements.JSONSchema202012();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { JSONSchemaElement, LinkDescriptionElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const jsonSchemaElement = new JSONSchemaElement();
const linkDescriptionElement = new LinkDescriptionElement();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-json-schema-2020-12/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isJSONSchemaElement, JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const jsonSchemaElement = new JSONSchemaElement();

isJSONSchemaElement(jsonSchemaElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-json-schema-2020-12/src/traversal/visitor.ts#L7) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-json-schema-2020-12/src/traversal/visitor.ts#L3).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { JSONSchemaElement, keyMap, getNodeType } from '@swagger-api/apidom-ns-json-schema-2020-12';

const element = new JSONSchemaElement();

const visitor = {
  JSONSchema202012Element(jsonSchemaElement) {
    console.dir(jsonSchemaElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { LinkDescriptionElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const object = {
  anchor: 'nodes/{thisNodeId}',
  anchorPointer: '#/relative/json/pointer',
};

LinkDescriptionElement.refract(object); // => LinkDescriptionElement({ anchor, anchorPointer })
```

**Refracting generic ApiDOM structures**:

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { LinkDescriptionElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const objectElement = new ObjectElement({
  anchor: 'nodes/{thisNodeId}',
  anchorPointer: '#/relative/json/pointer',
});

LinkDescriptionElement.refract(objectElement); // => LinkDescriptionElement({ anchor = 'nodes/{thisNodeId}', anchorPointer = '#/relative/json/pointer' })
```

### Refractor plugins

Refractors can accept plugins as a second argument of refract static method.

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { LinkDescriptionElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const objectElement = new ObjectElement({
  anchor: 'nodes/{thisNodeId}',
  anchorPointer: '#/relative/json/pointer',
});

const plugin = ({ predicates, namespace }) => ({
  name: 'plugin',
  pre() {
      console.dir('runs before traversal');
  },
  visitor: {
    LinkDescriptionElement(linkDescriptionElement) {
      linkDescriptionElement.anchorPointer = '#/relative/json/pointer/x';
    },
  },
  post() {
      console.dir('runs after traversal');
  },
});

LinkDescriptionElement.refract(objectElement, { plugins: [plugin] }); // => LinkDescriptionElement({ anchor = 'nodes/{thisNodeId}', anchorPointer = '#/relative/json/pointer/x' })
```

You can define as many plugins as needed to enhance the resulting namespaced ApiDOM structure.
If multiple plugins with the same visitor method are defined, they run in parallel (just like in Babel).

#### Replace Empty Element plugin

This plugin is specific to YAML 1.2 format, which allows defining key-value pairs with empty key,
empty value, or both. If the value is not provided in YAML format, this plugin compensates for
this missing value with the most appropriate semantic element type.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginReplaceEmptyElement, JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';

const yamlDefinition = `
$schema: 'https://json-schema.org/draft/2020-12/schema'
if:
`;
const apiDOM = await parse(yamlDefinition);
const jsonSchemaElement = JSONSchemaElement.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (JSONSchema202012Element
//   (MemberElement
//     (StringElement)
//     (StringElement))
//   (MemberElement
//     (StringElement)
//     (JSONSchema202012Element)))

// => without the plugin the result would be as follows:
// (JSONSchema202012Element
//   (MemberElement
//     (StringElement)
//     (StringElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [JSON Schema Object](https://json-schema.org/draft/2020-12/json-schema-core)
- [x] [Link Description Object](https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#rfc.section.6)



