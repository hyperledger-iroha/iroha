# @swagger-api/apidom-ns-api-design-systems

`@swagger-api/apidom-ns-api-design-systems` contains ApiDOM namespace specific to [API Design System Specification](https://apidesign.systems/specification/).

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-api-design-systems
```

## API Design Systems 2021-05-07 namespace

API Design Systems 2021-05-07 namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-api-design-systems/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import apiDesignSystemsNamespace from '@swagger-api/apidom-ns-api-design-systems';

const namespace = createNamespace(apiDesignSystemsNamespace);

const objectElement = new namespace.elements.Object();
const mainElement = new namespace.elements.Main();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { MainElement, InfoElement } from '@swagger-api/apidom-ns-api-design-systems';

const infoElement = new InfoElement();
const mainElement = new MainElement();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-api-design-systems/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isMainElement, MainElement } from '@swagger-api/apidom-ns-api-design-systems';

const mainElement = new MainElement();

isMainElement(mainElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-api-design-systems/src/traversal/visitor.ts#L16) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-api-design-systems/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { MainElement, keyMap, getNodeType } from '@swagger-api/apidom-ns-api-design-systems';

const element = new MainElement();

const visitor = {
  MainElement(mainElement) {
    console.dir(mainElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { InfoElement } from '@swagger-api/apidom-ns-api-design-systems';

const object = {
    title: 'my title',
    description: 'my description',
};

InfoElement.refract(object); // => InfoElement({ title, description })
```

**Refracting generic ApiDOM structures**:

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-api-design-systems';

const objectElement = new ObjectElement({
    title: 'my title',
    description: 'my description',
});

InfoElement.refract(objectElement); // => InfoElement({ title = 'my title', description = 'my description' })
```

### Refractor plugins

Refractors can accept plugins as a second argument of refract static method.

```js
import { ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-api-design-systems';

const objectElement = new ObjectElement({
    title: 'my title',
    description: 'my description',
});

const plugin = ({ predicates, namespace }) => ({
  name: 'plugin',
  pre() {
      console.dir('runs before traversal');
  },
  visitor: {
    InfoElement(infoElement) {
      infoElement.title = 'new title';
    },
  },
  post() {
      console.dir('runs after traversal');
  },
});

InfoElement.refract(objectElement, { plugins: [plugin] }); // => InfoElement({ title = 'new title', description = 'my description' })
```

You can define as many plugins as needed to enhance the resulting namespaced ApiDOM structure.
If multiple plugins with the same visitor method are defined, they run in parallel (just like in Babel).

#### OpenAPI 3.1 Standard Identifier plugins

This namespace comes with two refractor plugins specific to OpenAPI 3.1 specification and decorates significant
OpenAPI 3.1 elements with [Standard Identifiers](https://apidesign.systems/standards/).

```js
import { parse } from '@swagger-api/apidom-parser-adapter-json';
import {
  refractPluginOpenApi3_1StandardIdentifierSelectors,
  refractPluginOpenApi3_1StandardIdentifierAccessors,
} from '@swagger-api/apidom-ns-api-design-systems';
import { OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const jsonDefinition = `
{
  "openapi": "3.1.0",
  "paths": {
    "/path1": {
      "get": {},
      "put": {
        "parameters": [
          {
            "name": "X-Header",
            "in": "header",
            "description": "parameter3 description",
            "required": false,
            "deprecated": false,
            "allowEmptyValue": true
          }
        ],
        "requestBody": {
          "content": {
            "application/json": {
              "schema": {
                "type": "object"
              }
            }
          },
          "required": true
        },
        "responses": {
          "201": {
            "content": {
              "application/json": {}
            }
          }
        }
      },
      "parameters": [
        {
          "name": "X-Header2",
          "in": "header",
          "description": "parameter1 description",
          "allowEmptyValue": true
        }
      ]
    }
  }
}
`;
const apiDOM = await parse(jsonDefinition, { sourceMap: true });
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [
    refractPluginOpenApi3_1StandardIdentifierSelectors(),
    refractPluginOpenApi3_1StandardIdentifierAccessors(),
  ],
});
// => OperationElement now contains [['http', 'transaction']] under `ads-s-standard-identifier` key
// => OperationElement now contains followign structure under `ads-a-standard-identifier` key
// [
//   {
//     subject: ['http', 'request', 'method'],
//     value: 'get',
//   },
// ]
//
// other elements are decorated by different metadata as well
```

## Validation

This package supports validation API Design Systems definition against OpenAPI 3.1 definition.
Validator is in POC phase and there is limited support for `Scenario` objects. Validation
produces list of `Annotation` elements.

**API Design Systems definition**:

```yaml
version: "2021-05-07"
info:
  title: SmartBear API Guidelines
  description: |
    A machine and human readable version of the SmartBear API Style Guide aimed at promoting living API Style Governance across various tools and teams, leading to improved API quality.
    See the [SmartBear Standards and Guidelines](https://github.com/SmartBear/api-standards-and-guidelines) repo for a traditional view of the static guidelines.
principles:
  - iri: urn:apidesign.systems:principle:robustness
    level: must
  - iri: urn:apidesign.systems:principle:rmm:level2
    level: must
  - iri: urn:apidesign.systems:principle:rmm:level3
    level: should
standards:
  - iri: urn:ietf:rfc:6648
    level: must
  - iri: urn:ietf:rfc:7807
    level: must
  - iri: urn:ietf:rfc:7231
    level: should
  - iri: urn:ietf:rfc:6585
    level: may
  - iri: urn:ietf:rfc:5788
    level: may
  - iri: urn:ietf:draft:http-semantics
    level: may
scenarios:
  - description: SB-API-010 - Only apply standard HTTP methods
    when: [http, transaction]
    then:
      - subject: [http, request, method]
        level: may
        values:
          - get
          - post
          - put
          - patch
          - delete
```

**OpenAPI 3.1 definition**:

```json
{
  "openapi": "3.1.0",
  "paths": {
    "/path1": {
      "get": {},
      "put": {},
      "trace": {},
      "options": {}
    }
  }
}
```

**Validation**:

```js
import { parse as parseJSON } from '@swagger-api/apidom-parser-adapter-json';
import { parse as parseYAML } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';
import {
  refractPluginOpenApi3_1StandardIdentifierSelectors,
  refractPluginOpenApi3_1StandardIdentifierAccessors,
  MainElement,
  validateOpenAPI3_1,
} from '@swagger-api/apidom-ns-api-design-systems';

const apiDesignSystemsParseResult = await parseYAML(apiDesignSystemsDefinition);
const openAPIParseResult = await parseJSON(openAPIDefinition, { sourceMap: true });
const mainElement = MainElement.refract(apiDesignSystemsParseResult.result);
const  openapiElement = OpenApi3_1Element.refract(openAPIParseResult.result, {
  plugins: [
    refractPluginOpenApi3_1StandardIdentifierSelectors(),
    refractPluginOpenApi3_1StandardIdentifierAccessors(),
  ],
});
const annotations = validateOpenAPI3_1(mainElement, openapiElement);
// [
//   Annotation('"trace" not allowed for subject ["http","request","method"] on line 6, column 6', class=error),
//   Annotation('"options" not allowed for subject ["http","request","method"] on line 7, column 6', class=error),
// ]
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [Main](https://apidesign.systems/specification/#main)
- [x] [Info](https://apidesign.systems/specification/#info-object)
- [x] [Principle](https://apidesign.systems/specification/#principle)
- [x] [Standard](https://apidesign.systems/specification/#standard)
- [x] [Scenario](https://apidesign.systems/specification/#scenario)
- [x] [Requirement](https://apidesign.systems/specification/#requirement)
- [x] [Standard Identifier](https://apidesign.systems/specification/#standard-identifier-arraystring)
- [x] [Requirement Level](https://apidesign.systems/specification/#requirement-level-enum)
