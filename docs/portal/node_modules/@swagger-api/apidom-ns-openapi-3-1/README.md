# @swagger-api/apidom-ns-openapi-3-1

`@swagger-api/apidom-ns-openapi-3-1` contains ApiDOM namespace specific to [OpenApi 3.1.0 specification](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md).

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-openapi-3-1
```

## OpenAPI 3.1.0 namespace

OpenAPI 3.1.0 namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-openapi-3-1/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import openApi3_1Namespace from '@swagger-api/apidom-ns-openapi-3-1';

const namespace = createNamespace(openApi3_1Namespace);

const objectElement = new namespace.elements.Object();
const openApiElement = new namespace.elements.OpenApi3_1();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { OpenApi3_1Element, InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

const infoElement = new InfoElement();
const openApiElement = new OpenApi3_1Element();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-1/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isOpenApi3_1Element, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const openApiElement = new OpenApi3_1Element();

isOpenApi3_1Element(openApiElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-1/src/traversal/visitor.ts#L24) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-openapi-3-1/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { OpenApi3_1Element, keyMap, getNodeType } from '@swagger-api/apidom-ns-openapi-3-1';

const element = new OpenApi3_1Element();

const visitor = {
  OpenApi3_1Element(openApiElement) {
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
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

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
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

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
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

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
All the plugins available in `@swagger-api/apidom-ns-openapi-3-1` are idempotent and the normalization state is stored in the root `OpenApi3_1Element` in `<storageField>`. `<storageField>` can be customized in the plugin configuration (default: `x-normalized`).

#### Replace Empty Element plugin

This plugin is specific to YAML 1.2 format, which allows defining key-value pairs with empty key,
empty value, or both. If the value is not provided in YAML format, this plugin compensates for
this missing value with the most appropriate semantic element type.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginReplaceEmptyElement, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
info:
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (OpenApi3_1Element
//   (MemberElement
//     (StringElement)
//     (OpenapiElement))
//   (MemberElement
//     (StringElement)
//     (InfoElement)))

// => without the plugin the result would be as follows:
// (OpenApi3_1Element
//   (MemberElement
//     (StringElement)
//     (OpenapiElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

#### Normalize Operation.operationId fields plugin

Existing `Operation.operationId` fields are normalized into snake case form.
Operation Objects, that do not define operationId field, are left untouched.
Original operationId is stored in meta and as new `__originalOperationId` field.
This plugin also guarantees the uniqueness of all defined Operation.operationId fields,
and make sure Link.operationId fields are pointing to correct and normalized Operation.operationId fields.

```js
import { toValue } from '@swagger-api/apidom-core';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginNormalizeOperationIds, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
paths:
  /:
    get:
      operationId: get operation ^
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeOperationIds()],
});

toValue(openApiElement);
// =>
// {
//   "openapi": "3.1.0",
//   "paths": {
//     "/": {
//       "get": {
//         "operationId": "getoperation_"
//       }
//     }
//   }
// }
```
This plugin also accepts custom normalization function that will determine how normalized Operation.operationId fields
should look like.

```typescript
import { toValue } from '@swagger-api/apidom-core';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginNormalizeOperationIds, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
paths:
  /:
    get:
      operationId: get operation ^
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeOperationIds({
    operationIdNormalizer: (operationId: string, path: string, method: string): string => {
      // operationId - value of Original.operationId field
      // path - field pattern of Paths Object under which Path Item containing this Operation is registered
      // method - name of HTTP method under which the Operation is registered in Path Item
    },
  })],
});

toValue(openApiElement);
// =>
// {
//   "openapi": "3.1.0",
//   "paths": {
//     "/": {
//       "get": {
//         "operationId": "<normalized-operation-id>"
//       }
//     }
//   }
// }
```

#### Normalize Parameter Objects plugin

Duplicates Parameters from Path Items to Operation Objects using following rules:

- If a parameter is already defined at the Path Item, the new definition will override it but can never remove it
- The list MUST NOT include duplicated parameters
- A unique parameter is defined by a combination of a name and location.

```js
import { toValue } from '@swagger-api/apidom-core';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginNormalizeParameters, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
paths:
  /:
    parameters:
      - name: param1
        in: query
      - name: param2
        in: query
    get: {}
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeParameters()],
});

toValue(openApiElement);
// =>
// {
//   "openapi": "3.1.0",
//   "paths": {
//   "/": {
//     "parameters": [
//       {
//         "name": "param1",
//         "in": "query"
//       },
//       {
//         "name": "param2",
//         "in": "query"
//       }
//     ],
//     "get": {
//       "parameters": [
//          {
//            "name": "param1",
//            "in": "query"
//          },
//          {
//            "name": "param2",
//            "in": "query"
//          }
//        ],
//      }
//    }
// }
```

#### Normalize Security Requirements Objects plugin

`Operation.security` definition overrides any declared top-level security from OpenAPI.security field.
If Operation.security field is not defined, this field will inherit security from OpenAPI.security field.

```js
import { toValue } from '@swagger-api/apidom-core';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginNormalizeSecurityRequirements, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
security:
  - petstore_auth:
      - write:pets
      - read:pets
paths:
  /:
    get: {}
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeSecurityRequirements()],
});

toValue(openApiElement);
// =>
// {
//   "openapi": "3.1.0",
//   "security": [
//     {
//       "petstore_auth": [
//         "write:pets",
//         "read:pets"
//       ]
//     }
//   ],
//   "paths": {
//     "/": {
//       "get": {
//         "security": [
//           {
//             "petstore_auth": [
//               "write:pets",
//               "read:pets"
//             ]
//           }
//         ]
//       }
//     }
//   }
// }
```

#### Normalize Server Objects plugin

List of Server Objects can be defined in OpenAPI 3.1 on multiple levels:

- OpenAPI.servers
- PathItem.servers
- Operation.servers

If an alternative server object is specified at the Path Item Object level, it will override OpenAPI.servers.
If an alternative server object is specified at the Operation Object level, it will override PathItem.servers and OpenAPI.servers respectively.

```js
import { toValue } from '@swagger-api/apidom-core';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginNormalizeServers, OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';

const yamlDefinition = `
openapi: 3.1.0
servers:
 - url: https://example.com/
   description: production server
paths:
  /:
    get: {}
`;
const apiDOM = await parse(yamlDefinition);
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeServers()],
});

toValue(openApiElement);
// =>
// {
//   "openapi": "3.1.0",
//   "servers": [
//     {
//       "url": "https://example.com/",
//       "description": "production server"
//     }
//   ],
//   "paths": {
//     "/": {
//       "servers": [
//         {
//           "url": "https://example.com/",
//           "description": "production server"
//         }
//       ],
//       "get": {
//         "servers": [
//           {
//             "url": "https://example.com/",
//             "description": "production server"
//           }
//         ]
//       }
//     }
//   }
// }
```
#### Normalize Parameter examples plugin

`parameter.examples` and `parameter.example` override `parameter.schema.examples` and `parameter.schema.example` fields. The plugin overrides only the existing `parameter.schema.examples` and `parameter.schema.example`.

- Does not apply to parameters defined under `components`.
- `parameter.examples` has precedence over deprecated `parameter.example`.

```js 
import { toValue } from '@swagger-api/apidom-core';
import { OpenApi3_1Element, refractorPluginNormalizeParameterExamples } from '@swagger-api/apidom-ns-openapi-3-1';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';

const yamlDefinition = `
openapi: 3.1.0
paths:
  /:
    get:
      parameters:
          - in: query
            name: idempotent
            schema:
              type: number
              examples: [1]
            examples:
              example1:
                value: 2`

const apiDOM = await parse(yamlDefinition);

//default 
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeParameterExamples()],
});
toValue(openApiElement);
// =>
// {
//   openapi: '3.1.0',
//   paths: {
//     '/': {
//       get: {
//         parameters: [
//           {
//             in: 'query',
//             name: 'idempotent',
//             schema: { type: 'number', examples: [ 2 ] },
//             examples: { example1: { value: 2 } }
//           }
//         ]
//       }
//     }
//   },
//   'x-normalized': { 'parameter-examples': [ '/paths/~1/get/parameters/0' ] }
// }

// custom storage field name
const openApiElementWithCustomField = OpenApi3_1Element.refract(apiDOM.result, {
  plugins: [refractorPluginNormalizeParameterExamples({ storageField: '$$my-normalized' })],
});
toValue(openApiElementWithCustomField);
// =>
// {
//   openapi: '3.1.0',
//   paths: {
//     '/': {
//       get: {
//         parameters: [
//           {
//             in: 'query',
//             name: 'idempotent',
//             schema: { type: 'number', examples: [2] },
//             examples: { example1: { value: 2 } }
//           }
//         ]
//       }
//     }
//   },
//   '$$my-normalized': { 'parameter-examples': [ '/paths/~1/get/parameters/0' ] }
// }
```

#### Normalize Header examples plugin

`Header.examples` and `header.example` override `header.schema.examples` and `header.schema.example` fields. The plugin overrides only the existing `header.schema.examples` and `header.schema.example`.

- Does not apply to headers defined under `components`.
- `header.examples` has precedence over deprecated `header.example`.

```js 
import { toValue } from '@swagger-api/apidom-core';
import { OpenApi3_1Element, refractorPluginNormalizeHeaderExamples } from '@swagger-api/apidom-ns-openapi-3-1';
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';

const yamlDefinition = `
openapi: 3.1.0
paths:
  /:
    get:
      responses:
        "200":
          headers:
            content-type:
              schema:
                type: number
                example: 1
              examples:
                example1:
                  value: 2
`
const apiDOM = await parse(yamlDefinition);

// default
const openApiElement = OpenApi3_1Element.refract(apiDOM.result, {
   plugins: [refractorPluginNormalizeHeaderExamples()],
});
toValue(openApiElement);
// =>
// {
//   openapi: '3.1.0',
//   paths: {
//     '/': {
//       get: {
//         responses: {
//           '200': {
//             headers: {
//               'content-type': {
//                 schema: { type: 'number', example: 2 },
//                 examples: { example1: { value: 2 } }
//               }
//             }
//           }
//         }
//       }
//     }
//   },
//   'x-normalized': {
//     'header-examples': [ '/paths/~1/get/responses/200/headers/content-type' ]
//   }
// }

// custom storage field name

const openApiElementWithCustomField = OpenApi3_1Element.refract(apiDOM.result, {
   plugins: [refractorPluginNormalizeHeaderExamples({ storageField: '$$normalized' })],
});
toValue(openApiElementWithCustomField);
// =>
// {
//   openapi: '3.1.0',
//   paths: {
//     '/': {
//       get: {
//         responses: {
//           '200': {
//             headers: {
//               'content-type': {
//                 schema: { type: 'number', example: 2 },
//                 examples: { example1: { value: 2 } }
//               }
//             }
//           }
//         }
//       }
//     }
//   },
//   '$$normalized': {
//     'header-examples': [ '/paths/~1/get/responses/200/headers/content-type' ]
//   }
// }
```

#### Normalize Discriminator mapping plugin
This plugin normalizes the `discriminator.mapping` field in a Schema Object by:

- Converting mapping values into inline Schema Objects when possible.
- Adding missing mapping entries based on the schema's `oneOf`, `anyOf`, or a prepared `allOf` mapping. The `allOf` mapping 
  is created based on the schemas defined in `components.schemas` during dereferencing.

The `discriminator.mapping` field is not modified by the plugin.

This plugin is intended to run on dereferenced OpenAPI 3.1 documents. During dereferencing Schema Objects are annotated with meta properties and the `allOf` mapping is created for Schema Objects defined in `components.schemas`. 

```json 

// fixture-example.json
{
  "openapi": "3.1.0",
  "components": {
    "schemas": {
      "MyResponse": {
        "type": "object",
        "oneOf": [
          {
            "$ref": "#/components/schemas/Cat"
          },
          {
            "$ref": "#/components/schemas/Dog"
          }
        ],
        "discriminator": {
          "propertyName": "petType"
        }
      },
      "Pet": {
        "type": "object",
        "properties": {
          "petType": {
            "type": "string"
          }
        }
      },
      "Cat": {
        "allOf": [
          {
            "$ref": "#/components/schemas/Pet"
          },
          {
            "type": "object",
            "properties": {
              "meows": {
                "type": "boolean"
              }
            }
          }
        ]
      },
      "Dog": {
        "allOf": [
          {
            "$ref": "#/components/schemas/Pet"
          },
          {
            "type": "object",
            "properties": {
              "barks": {
                "type": "boolean"
              }
            }
          }
        ]
      }
    }
  }
}
```

```js 
import { toValue, dispatchRefractorPlugins } from '@swagger-api/apidom-core';
import { dereference } from '@swagger-api/apidom-reference';
import FileResolver from '@swagger-api/apidom-reference/resolve/resolvers/file';
import {
  createToolbox,
  refractorPluginNormalizeDiscriminatorMapping,
  keyMap,
  getNodeType,
  mediaTypes,
} from '@swagger-api/apidom-ns-openapi-3-1';
const uri = 'path/to/fixture-example.json'; // the arbitrary file name shown above

// 1) dereference the document to annotate schemas with required metadata
const dereferenced = await dereference(uri, {
  parse: { mediaType: mediaTypes.latest('json') },
  resolve: {
    baseURI: uri,
    resolvers: [ new FileResolver({ fileAllowList: [/\.json$/] }) ],
  },
  dereference: { strategyOpts: { 'openapi-3-1': { dereferenceDiscriminatorMapping: true } } },
});

// 2) dispatch the plugin and pass the same baseURI
const normalized = dispatchRefractorPlugins(
  dereferenced.result,
  [refractorPluginNormalizeDiscriminatorMapping({ baseURI: uri })],
  { toolboxCreator: createToolbox, visitorOptions: { keyMap, nodeTypeGetter: getNodeType } }
);
toValue(normalized)

// => 
// {
//   openapi: '3.1.0',
//   components: {
//     schemas: {
//       MyResponse: {
//         type: 'object',
//         oneOf: [
//           {
//             allOf: [
//               {
//                 type: 'object',
//                 properties: { petType: { type: 'string' } }
//               },
//               {
//                 type: 'object',
//                 properties: { meows: { type: 'boolean' } }
//               }
//             ]
//           },
//           {
//             allOf: [
//               {
//                 type: 'object',
//                 properties: { petType: { type: 'string' } }
//               },
//               {
//                 type: 'object',
//                 properties: { barks: { type: 'boolean' } }
//               }
//             ]
//           }
//         ],
//         discriminator: {
//           propertyName: 'petType',
//           'x-normalized-mapping': {
//             Cat: {
//               allOf: [
//                 {
//                   type: 'object',
//                   properties: { petType: { type: 'string' } }
//                 },
//                 {
//                   type: 'object',
//                   properties: { meows: { type: 'boolean' } }
//                 }
//               ]
//             },
//             Dog: {
//               allOf: [
//                 {
//                   type: 'object',
//                   properties: { petType: { type: 'string' } }
//                 },
//                 {
//                   type: 'object',
//                   properties: { barks: { type: 'boolean' } }
//                 }
//               ]
//             }
//           }
//         }
//       },
//       Pet: { type: 'object', properties: { petType: { type: 'string' } } },
//       Cat: {
//         allOf: [
//           {
//             type: 'object',
//             properties: { petType: { type: 'string' } }
//           },
//           {
//             type: 'object',
//             properties: { meows: { type: 'boolean' } }
//           }
//         ]
//       },
//       Dog: {
//         allOf: [
//           {
//             type: 'object',
//             properties: { petType: { type: 'string' } }
//           },
//           {
//             type: 'object',
//             properties: { barks: { type: 'boolean' } }
//           }
//         ]
//       }
//     }
//   },
//   'x-normalized': { 'discriminator-mapping': [ '/components/schemas/MyResponse' ] }
// }
```

## Implementation progress

Only fully implemented specification objects should be checked here.

- [x] [OpenAPI Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#openapi-object)
- [x] [Info Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#info-object)
- [x] [Contact Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#contact-object)
- [x] [License Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#license-object)
- [x] [Server Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#server-object)
- [x] [Server Variable Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#server-variable-object)
- [x] [Components](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#components-object)
- [x] [Paths Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#paths-object)
- [x] [Path Item Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#path-item-object)
- [x] [Operation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#operation-object)
- [x] [External Documentation Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#external-documentation-object)
- [x] [Parameter Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#parameter-object)
- [x] [Request Body Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#request-body-object)
- [x] [Media Type Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#media-type-object)
- [x] [Encoding Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#encoding-object)
- [x] [Responses Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#responses-object)
- [x] [Callback Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#callback-object)
- [x] [Example Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#example-object)
- [x] [Link Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#link-object)
- [x] [Header Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#header-object)
- [x] [Tag Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#tag-object)
- [x] [Reference Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#reference-object)
- [x] [Schema Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#schema-object)
- [x] [Discriminator Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#discriminator-object)
- [x] [XML Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#xml-object)
- [x] [Security Scheme Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#security-scheme-object)
- [x] [OAuth Flows Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#oauth-flows-object)
- [x] [OAuth Flow Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#oauth-flow-object)
- [x] [Security Requirement Object](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#security-requirement-object)
- [x] [Specification extensions](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#specification-extensions)
