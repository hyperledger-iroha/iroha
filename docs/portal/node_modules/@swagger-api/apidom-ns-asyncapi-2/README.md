# @swagger-api/apidom-ns-asyncapi-2

`@swagger-api/apidom-ns-asyncapi-2` contains ApiDOM namespace supports following AsyncAPI specification versions:

- [AsyncAPI 2.6.0 specification](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md)
- [AsyncAPI 2.5.0 specification](https://github.com/asyncapi/spec/blob/v2.5.0/spec/asyncapi.md)
- [AsyncAPI 2.4.0 specification](https://github.com/asyncapi/spec/blob/v2.4.0/spec/asyncapi.md)
- [AsyncAPI 2.3.0 specification](https://github.com/asyncapi/spec/blob/v2.3.0/spec/asyncapi.md)
- [AsyncAPI 2.2.0 specification](https://github.com/asyncapi/spec/blob/v2.2.0/spec/asyncapi.md)
- [AsyncAPI 2.1.0 specification](https://github.com/asyncapi/spec/blob/v2.1.0/spec/asyncapi.md)
- [AsyncAPI 2.0.0 specification](https://github.com/asyncapi/spec/blob/2.0.0/versions/2.0.0/asyncapi.md)


## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ns-asyncapi-2
```

## AsyncApi 2.x.y namespace

AsyncApi 2.x.y namespace consists of [number of elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ns-asyncapi-2/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';
import asyncApi2Namespace from '@swagger-api/apidom-ns-asyncapi-2';

const namespace = createNamespace(asyncApi2Namespace);

const objectElement = new namespace.elements.Object();
const asyncApiElement = new namespace.elements.AsyncApi2();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

Elements from the namespace can also be used directly by importing them.

```js
import { AsyncApi2Element, InfoElement } from '@swagger-api/apidom-ns-asyncapi-2';

const infoElement = new InfoElement();
const asyncApiElement = new AsyncApi2Element();
```

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-asyncapi-2/src/predicates.ts)
for all higher order elements that are part of this namespace.

```js
import { isAsyncApi2Element, AsyncApi2Element } from '@swagger-api/apidom-ns-asyncapi-2';

const asyncApiElement = new AsyncApi2Element();

isAsyncApi2Element(asyncApiElement); // => true
```

## Traversal

Traversing ApiDOM in this namespace is possible by using `visit` function from `apidom-core` package.
This package comes with its own [keyMap](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-asyncapi-2/src/traversal/visitor.ts#L16) and [nodeTypeGetter](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ns-asyncapi-2/src/traversal/visitor.ts#L6).
To learn more about these `visit` configuration options please refer to [@swagger-api/apidom-ast documentation](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/README.md#visit).

```js
import { visit } from '@swagger-api/apidom-core';
import { AsyncApi2Element, keyMap, getNodeType } from '@swagger-api/apidom-ns-asyncapi-2';

const element = new AsyncApi2Element();

const visitor = {
  AsyncApi2Element(asyncApiElement) {
    console.dir(asyncApiElement);
  },
};

visit(element, visitor, { keyMap, nodeTypeGetter: getNodeType });
```

## Refractors

Refractor is a special layer inside the namespace that can transform either JavaScript structures
or generic ApiDOM structures into structures built from elements of this namespace.

**Refracting JavaScript structures**:

```js
import { InfoElement } from '@swagger-api/apidom-ns-asyncapi-2';

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
import { InfoElement } from '@swagger-api/apidom-ns-asyncapi-2';

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
import { InfoElement } from '@swagger-api/apidom-ns-asyncapi-2';

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
      infoElement.version = '2.6.0';
    },
  },
  post() {
      console.dir('runs after traversal');
  },
});

InfoElement.refract(objectElement, { plugins: [plugin] }); // => InfoElement({ title = 'my title', description = 'my description', version = '2.6.0' })
```

You can define as many plugins as needed to enhance the resulting namespaced ApiDOM structure.
If multiple plugins with the same visitor method are defined, they run in parallel (just like in Babel).

#### Replace Empty Element plugin

This plugin is specific to YAML 1.2 format, which allows defining key-value pairs with empty key,
empty value, or both. If the value is not provided in YAML format, this plugin compensates for
this missing value with the most appropriate semantic element type.

```js
import { parse } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import { refractorPluginReplaceEmptyElement, AsyncApi2Element } from '@swagger-api/apidom-ns-asyncapi-2';

const yamlDefinition = `
asyncapi: 2.6.0
info:
`;
const apiDOM = await parse(yamlDefinition);
const asyncApiElement = AsyncApi2Element.refract(apiDOM.result, {
  plugins: [refractorPluginReplaceEmptyElement()],
});

// =>
// (AsyncApi2Element
//   (MemberElement
//     (StringElement)
//     (AsyncApiVersionElement))
//   (MemberElement
//     (StringElement)
//     (InfoElement)))

// => without the plugin the result would be as follows:
// (AsyncApi2Element
//   (MemberElement
//     (StringElement)
//     (AsyncApiVersionElement))
//   (MemberElement
//     (StringElement)
//     (StringElement)))
```

## Implementation progress

Only fully implemented specification objects should be checked here.

### Specification Objects

- [x] [AsyncAPI Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#asyncapi-object)
- [x] [AsyncAPI Version String](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#asyncapi-version-string)
- [x] [Identifier](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#identifier)
- [x] [Info Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#info-object)
- [x] [Contact Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#contact-object)
- [x] [License Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#license-object)
- [x] [Servers Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#servers-object)
- [x] [Server Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#server-object)
- [x] [Server Variable Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#server-variable-object)
- [x] [Default Content Type](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#default-content-type)
- [x] [Channels Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#channels-object)
- [x] [Channel Item Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#channel-item-object)
- [x] [Operation Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#operation-object)
- [x] [Operation Trait Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#operation-trait-object)
- [x] [Message Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#message-object)
- [x] [Message Trait Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#message-trait-object)
- [x] [Message Example Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#-message-example-object)
- [x] [Tags Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#tags-object)
- [x] [Tag Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#tag-object)
- [x] [External Documentation Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#external-documentation-object)
- [x] [Components Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#components-object)
- [x] [Reference Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#reference-object)
- [x] [Schema Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#schema-object)
- [x] [Security Scheme Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#security-scheme-object)
- [x] [Security Requirement Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#security-requirement-object)
- [x] [OAuth Flows Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#oauth-flows-object)
- [x] [OAuth Flow Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#oauth-flow-object)
- [x] [Server Bindings Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#server-bindings-object)
- [x] [Parameters Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#parameters-object)
- [x] [Parameter Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#parameter-object)
- [x] [Channel Bindings Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#channel-bindings-object)
- [x] [Operation Bindings Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#operation-bindings-object)
- [x] [Message Bindings Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#message-bindings-object)
- [x] [Correlation ID Object](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#correlation-id-object)
- [x] [Specification Extension](https://github.com/asyncapi/spec/blob/v2.6.0/spec/asyncapi.md#specification-extensions)

### Binding Objects

#### HTTP Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/http/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/http/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/http/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/http/README.md#message-binding-object)

#### WebSockets Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/websockets/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/websockets/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/websockets/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/websockets/README.md#message-binding-object)

#### Kafka Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/kafka/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/kafka/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/kafka/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/kafka/README.md#message-binding-object)

#### Anypoint MQ Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/tree/master/anypointmq#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/tree/master/anypointmq#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/tree/master/anypointmq#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/tree/master/anypointmq#message-binding-object)

#### AMQP 0-9-1 Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp/README.md#message-binding-object)

#### AMQP 1.0 Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp1/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp1/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp1/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/amqp1/README.md#message-binding-object)

#### MQTT Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt/README.md#message-binding-object)

#### MQTT 5 Bindings

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt5/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt5/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt5/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/mqtt5/README.md#message-binding-object)

#### NATS

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/nats/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/nats/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/nats/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/nats/README.md#message-binding-object)

#### JMS

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/jms/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/jms/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/jms/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/jms/README.md#message-binding-object)

#### SNS

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/sns/2.x.x/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/sns/2.x.x/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/sns/2.x.x/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/sns/2.x.x/README.md#message-binding-object)

#### Solace

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/solace/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/solace/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/solace/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/solace/README.md#message-binding-object)

#### SQS

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/sqs/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/sqs/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/sqs/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/sqs/README.md#message-binding-object)

#### STOMP

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/blob/master/stomp/README.md#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/blob/master/stomp/README.md#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/blob/master/stomp/README.md#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/blob/master/stomp/README.md#message-binding-object)

#### Redis

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/tree/master/redis#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/tree/master/redis#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/tree/master/redis#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/tree/master/redis#message-binding-object)

#### Mercure

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/tree/master/mercure#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/tree/master/mercure#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/tree/master/mercure#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/tree/master/mercure#message-binding-object)

#### IBM MQ

- [x] [Server Binding Object](https://github.com/asyncapi/bindings/tree/master/ibmmq#server-binding-object)
- [x] [Channel Binding Object](https://github.com/asyncapi/bindings/tree/master/ibmmq#channel-binding-object)
- [x] [Operation Binding Object](https://github.com/asyncapi/bindings/tree/master/ibmmq#operation-binding-object)
- [x] [Message Binding Object](https://github.com/asyncapi/bindings/tree/master/ibmmq#message-binding-object)
