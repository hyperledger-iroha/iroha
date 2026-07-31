# @swagger-api/apidom-ast

`@swagger-api/apidom-ast` contains tools necessary for parsing stage of ApiDOM, specifically for syntactic analysis.
Syntactic analysis will take a stream of tokens and turn it into an AST representation.
Using the information in the tokens, this phase will reformat them as an AST which represents
the structure of input string in a way that makes it easier to work with.

`@swagger-api/apidom-ast` currently contains AST nodes for [JSON](https://www.json.org/json-en.html) and [YAML 1.2](https://yaml.org/spec/1.2/spec.html) formats.

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-ast
```

## Base AST Nodes

Base AST nodes are nodes that are supplementary to any specific AST nodes.
Having standardized AST of various formats (JSON/YAML) allows us to have common
syntactic analysis or transformation algorithms.
These nodes includes [Error](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/Error.ts), [Literal](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/Literal.ts), [Position](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/Position.ts), etc...

Along with base nodes there are [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/predicates.ts) that can assert on these nodes.

## JSON AST Nodes

Convenient for low lever CST parsers that don't come with it's onw AST nodes.
You can find list of JSON AST nodes in this [directory](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ast/src/json/nodes).

## YAML AST Nodes

Convenient for low lever CST parsers that don't come with it's onw AST nodes.
You can find list of YAML AST nodes in this [directory](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ast/src/yaml/nodes).
As YAML is very complex format, along with nodes we also expose implementation of YAML [Failsafe](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ast/src/yaml/schemas/failsafe) and [JSON](https://github.com/swagger-api/apidom/tree/main/packages/apidom-ast/src/yaml/schemas/json) schemas
along with [formatters for canonical block scalars](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/yaml/schemas/canonical-format.ts).

## Traversal

`@swagger-api/apidom-ast` comes with its own traversal algorithm convenient for traversing [CST](https://en.wikipedia.org/wiki/Parse_tree) or [AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree).

### visit

[visit](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L442) will walk through an CST/AST using a depth first traversal, calling
the visitor's enter function at each node in the traversal, and calling the
leave function after visiting that node and all of its child nodes.

By returning different values from the enter and leave functions, the
behavior of the visitor can be altered, including skipping over a sub-tree of
the Node (by returning false), editing the Node Tree by returning a value or null
to remove the value, or to stop the whole traversal by returning [BREAK](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L52).

When using `visit` to edit an Node Tree, the original Node Tree will not be modified, and
a new version of the Node Tree with the changes applied will be returned from the
visit function.

```js
import { visit } from '@swagger-api/apidom-ast';

const tree = {
    type: 'root',
    children: [
      {
          type: 'child',
          value: 'this is child node',
          children: [],
      },
    ],
};

const keyMap = {
    root: ['children'],
};

const visitor = {
    child(node) {
        console.dir(node.value); // => 'this is child node'
    },
};

const newTree = visit(tree, visitor, { keyMap }); // => tree{...}
```

Notice how we used 3rd parameter to `visit` function. Actually it can consume number of configuration
options which can change its behavior.

Configuration option | Type | Default | Description
--- | --- | --- | ---
<a name="keyMap"></a>`keyMap` | `Object` | `null` | Defines how nodes map to it's children.
<a name="state"></a>`state` | `Object` | `{}` | Additional state that is provided to the visitor. State is merged inti visitor object in following manner: `Object.assign(visitor, state)`
<a name="breakSymbol"></a>`breakSymbol` | `Object` | `{}` | Defines a symbol that can break the traversal. Symbol is compared by strict equality (`===`).
<a name="visitFnGetter"></a>`visitFnGetter` | `Function` | [getVisitFn](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L12) | Function that extract appropriate method from the visitor given specific Node type.
<a name="nodeTypeGetter"></a>`nodeTypeGetter` | `Function` | [getNodeType](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L57) | Node type extractor function.
<a name="nodePredicate"><a/>`nodePredicate` | `Function` | [isNode](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L62) | Predicate that checks if particular Node can be really considered a Node.
<a name="detectCycles"><a/>`detectCycles` | `Boolean` | `true` | If the structure that needs to be traversed is represented as directed cyclic graph, `visit` will skip Nodes that have already been traversed to avoid infinite recursion.


