# @swagger-api/apidom-ns-arazzo-1

`@swagger-api/apidom-ns-arazzo-1` contains ApiDOM namespace specific to [Arazzo 1.0.1 specification](https://spec.openapis.org/arazzo/latest.html#version-1-0-1).

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-arazzo-1
```

## Arazzo 1.0.1 namespace

Arazzo 1.0.1 namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-arazzo-1/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import arazzo1Namespace from '@swagger-api/apidom-ns-arazzo-1';

const namespace = createNamespace(arazzo1Namespace);

const objectElement = new namespace.elements.Object();
const arazzoElement = new namespace.elements.ArazzoSpecification1();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { ArazzoSpecification1Element, InfoElement } from '@swagger-api/apidom-ns-arazzo-1';

const infoElement = new InfoElement();
const arazzoElement = new ArazzoSpecification1Element();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-arazzo-1/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isArazzoSpecification1Element, ArazzoSpecification1Element } from '@swagger-api/apidom-ns-arazzo-1';

const arazzoElement = new ArazzoSpecification1Element();

isArazzoSpecification1Element(arazzoElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-arazzo-1/src/traversal/visitor.ts#L18) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-arazzo-1/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { ArazzoSpecification1Element, keyMap, getNodeType } from '@swagger-api/apidom-ns-arazzo-1';

const element = new ArazzoSpecification1Element();

const visitor = {
  ArazzoSpecification1Element(arazzoElement) {
    console.dir(arazzoElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { InfoElement } from '@swagger-api/apidom-ns-arazzo-1';

const object = {
    title: 'my title',
    summary: 'my summary',
    description: 'my description',
    version: '0.1.0',
};

InfoElement.refract(object); // => InfoElement({ title, summary, description, version })
```

**Refracting generic ApiDOM structures**:

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-arazzo-1';

const objectElement = new ObjectElement({
    title: 'my title',
    summary: 'my summary',
    description: 'my description',
    version: '0.1.0',
});

InfoElement.refract(objectElement); // => InfoElement({ title = 'my title', summary = 'my summary', description = 'my description', version = '0.1.0' })
```

### Refractor plugins

Refractors can accept plugins as a second argument of refract static method.

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-arazzo-1';

const objectElement = new ObjectElement({
    title: 'my title',
    summary: 'my summary',
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
import { refractorPluginReplaceEmptyElement, ArazzoSpecification1Element } from '@swagger-api/apidom-ns-arazzo-1';

const yamlDefinition = `
arazzo: 1.0.1
info:
`;
const apiDOM = await parse(yamlDefinition);
const arazzoElement = ArazzoSpecification1Element.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (ArazzoSpecification1Element
//   (MemberElement
//     (StringElement)
//     (StringElement))
//   (MemberElement
//     (StringElement)
//     (InfoElement)))

// => without the plugin the result would be as follows:
// (ArazzoSpecification1Element
//   (MemberElement
//     (StringElement)
//     (StringElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [Arazzo Specification Object](https://spec.openapis.org/arazzo/latest.html#arazzo-specification-object)
- [x] [Info Object](https://spec.openapis.org/arazzo/latest.html#info-object)
- [x] [Source Description Object](https://spec.openapis.org/arazzo/latest.html#source-description-object)
- [x] [Workflow Object](https://spec.openapis.org/arazzo/latest.html#workflow-object)
- [x] [Step Object](https://spec.openapis.org/arazzo/latest.html#step-object)
- [x] [Parameter Object](https://spec.openapis.org/arazzo/latest.html#parameter-object)
- [x] [Success Action Object](https://spec.openapis.org/arazzo/latest.html#success-action-object)
- [x] [Failure Action Object](https://spec.openapis.org/arazzo/latest.html#failure-action-object)
- [x] [Component Object](https://spec.openapis.org/arazzo/latest.html#components-object)
- [x] [Reusable Object](https://spec.openapis.org/arazzo/latest.html#reusable-object)
- [x] [Criterion Object](https://spec.openapis.org/arazzo/latest.html#criterion-object)
- [x] [Criterion Expression Type Object](https://spec.openapis.org/arazzo/latest.html#criterion-expression-type-object)
- [x] [Request Body Object](https://spec.openapis.org/arazzo/latest.html#request-body-object)
- [x] [Reusable Object](https://spec.openapis.org/arazzo/latest.html#reusable-object)
- [x] [JSON Schema](https://json-schema.org/specification-links#2020-12)
- [x] [Specification extensions](https://spec.openapis.org/arazzo/latest.html#specification-extensions)
