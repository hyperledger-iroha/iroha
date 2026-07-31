# @swagger-api/apidom-core

`apidom-core` is a package that contains tools for manipulating the ApiDOM structures.

## Installation

You can install this package via [npm CLI](https://docs.npmjs.com/cli) by running the following command:

```sh
 $ npm install @swagger-api/apidom-core
```

---

## Base namespace

Base namespace consists of [three higher order elements](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core/src/elements) implemented on top
of [primitive ones](https://github.com/refractproject/minim/tree/master/lib/primitives).

```js
import { createNamespace } from '@swagger-api/apidom-core';

const namespace = createNamespace();

const objectElement = new namespace.elements.Object();
const commentElement = new namespace.elements.Comment();
```

It's possible to create namespace instances using another namespaces.

```js
import { createNamespace } from '@swagger-api/apidom-core';
import openApi3_1Namespace from '@swagger-api/apidom-ns-openapi-3-1';

const namespace = createNamespace(openApi3_1Namespace);

const objectElement = new namespace.elements.Object();
const openApiElement = new namespace.elements.OpenApi3_1();
```

When namespace instance is created in this way, it will extend the base namespace
with the namespace provided as an argument.

---

## Predicates

This package exposes [predicates](https://github.com/swagger-api/apidom/blob/main/packages/apidom-core/src/predicates/index.ts)
for all primitive elements and all higher order elements that are part of the base namespace.

```js
import { CommentElement, isCommentElement } from '@swagger-api/apidom-core';

const commentElement = new CommentElement();

isCommentElement(commentElement); // => true
```

[Predicate helpers](https://github.com/swagger-api/apidom/blob/main/packages/apidom-core/src/predicates/helpers.ts)
helps in building predicates for this and other packages.

```js
import { createPredicate } from '@swagger-api/apidom-core';

const isMyElement = createPredicate(
  ({ hasBasicElementProps, isElementType, primitiveEq }) => {
    return (element) =>
      element instanceof MyElement ||
      (hasBasicElementProps(element) && isElementType('myElement', element) && primitiveEq('object', element));
  },
);
```

---

## Transcluder

Transclusion is the inclusion of one ApiDOM fragment into another ApiDOM fragment.
Our [transcluder](https://github.com/swagger-api/apidom/tree/main/packages/apidom-core/src/transcluder) does exactly that and is based on mutating algorithm.

```js
import { transclude, ArrayElement, NumberElement } from '@swagger-api/apidom-core';

const element = new ArrayElement([1, 2, 3]);
const search = element.get(1);
const replace = new NumberElement(4);

transclude(search, replace, element); // => ArrayElement<[1, 4, 3]>
```

When multiple transclusions are going to be performed use [Transcluder class](https://github.com/swagger-api/apidom/blob/main/packages/apidom-core/src/transcluder/Transcluder.ts)
for optimal performance.

```js
import { Transcluder, ArrayElement, NumberElement } from '@swagger-api/apidom-core';

const element = new ArrayElement([1, 2, 3]);
const search = element.get(1);
const replace = new NumberElement(4);
const transcluder = new Transcluder({ element });

transcluder.transclude(search, replace); // => ArrayElement<[1, 4, 3]>
```

---

## Shallow merging

`mergeRight` and `mergeLeft` functions merge members of two or more ObjectElements shallowly
and handles shallow merging of ArrayElements as well.

### API

#### mergeRight(target, source, [options])

Merges two ApiDOM elements target and source shallowly, returning a new merged ApiDOM element with the elements
from both target and source. If an element at the same key is present for both target and source,
the value from source will appear in the result. Merging creates a new ApiDOM element,
so that neither target nor source is modified (operation is immutable).

```js
import { mergeRight, ObjectElement } from '@swagger-api/apidom-core';

const x = new ObjectElement({
  foo: { bar: 3 },
});

const y = new ObjectElement({
  foo: { baz: 4 },
  quux: 5,
});

const output = mergeRight(x, y);
// =>
// ObjectElement({
//   foo: ObjectElement({
//     baz: 4,
//   }),
//   quux: 5,
// })
```

#### mergeRight.all([element1, element2, ...], [options])

Merges shallowly any number of ApiDOM elements into a single ApiDOM element.

```js
import { mergeRight, ObjectElement } from '@swagger-api/apidom-core';

const foobar = new ObjectElement({ foo: { bar: 3 } });
const foobaz = new ObjectElement({ foo: { baz: 4 } });
const bar = new ObjectElement({ bar: 'yay!' });

const output = mergeRight.all([ foobar, foobaz, bar ]);
// => ObjectElement({ foo: { baz: 4 }, bar: 'yay!' })
```

#### mergeLeft(source, target, [options])

Merges two ApiDOM elements source and target shallowly, returning a new merged ApiDOM element with the elements
from both target and source. If an element at the same key is present for both target and source,
the value from source will appear in the result. Merging creates a new ApiDOM element,
so that neither target nor source is modified (operation is immutable).

```js
import { mergeLeft, ObjectElement } from '@swagger-api/apidom-core';

const x = new ObjectElement({
  foo: { bar: 3 },
});

const y = new ObjectElement({
  foo: { baz: 4 },
  quux: 5,
});

const output = mergeLeft(x, y);
// =>
// ObjectElement({
//   foo: ObjectElement({
//     bar: 3,
//   }),
//   quux: 5,
// })
```

#### mergeLeft.all([element1, element2, ...], [options])

Merges shallowly any number of ApiDOM elements into a single ApiDOM element.

```js
import { mergeLeft, ObjectElement } from '@swagger-api/apidom-core';

const foobar = new ObjectElement({ foo: { bar: 3 } });
const foobaz = new ObjectElement({ foo: { baz: 4 } });
const bar = new ObjectElement({ bar: 'yay!' });

const output = mergeLeft.all([ foobar, foobaz, bar ]);
// => ObjectElement({ foo: { baz: 3 }, bar: 'yay!' })
```

### Shallow merge Options

`mergeRight` and `mergeLeft` take the same options as [deepmerge](#deepmerge-options), except for `customMerge` and `clone`.

## Deep merging

`deepmerge` functions merged members of two or more ObjectElements deeply
and handles deep merging of ArrayElements as well. This deep merge implementation
is a functional equivalent of [deepmerge](https://www.npmjs.com/package/deepmerge)
that works equivalently on ApiDOM structures.

### API

#### deepmerge(target, source, [options])

Merges two ApiDOM elements target and source deeply, returning a new merged ApiDOM element with the elements
from both target and source. If an element at the same key is present for both target and source,
the value from source will appear in the result. Merging creates a new ApiDOM element,
so that neither target nor source is modified (operation is immutable).

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const x = new ObjectElement({
  foo: { bar: 3 },
  array: [
    {
      does: 'work',
      too: [1, 2, 3],
    },
  ],
});

const y = new ObjectElement({
  foo: { baz: 4 },
  quux: 5,
  array: [
    {
      does: 'work',
      too: [4, 5, 6],
    },
    {
      really: 'yes',
    },
  ],
});

const output = deepmerge(x, y);
// =>
// ObjectElement({
//   foo: ObjectElement({
//     bar: 3,
//     baz: 4,
//   }),
//   array: ArrayElement([
//     ObjectElement({
//       does: 'work',
//       too: [1, 2, 3],
//     }),
//     ObjectElement({
//       does: 'work',
//       too: [4, 5, 6],
//     }),
//     ObjectElement({
//       really: 'yes',
//     }),
//   ]),
//   quux: 5,
// })
```

#### deepmerge.all([element1, element2, ...], [options])

Merges deeply any number of ApiDOM elements into a single ApiDOM element.

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const foobar = new ObjectElement({ foo: { bar: 3 } });
const foobaz = new ObjectElement({ foo: { baz: 4 } });
const bar = new ObjectElement({ bar: 'yay!' });

const output = deepmerge.all([ foobar, foobaz, bar ]);
// => ObjectElement({ foo: { bar: 3, baz: 4 }, bar: 'yay!' })
```

### Deepmerge Options

#### arrayElementMerge

There are multiple ways to merge two ArrayElements, below are a few examples, but you can also create your own custom function.

Your `arrayElementMerge` function will be called with three arguments: a `target` ArrayElement, the `source` ArrayElement,
and an `options` object.

```js
import { deepmerge, ArrayElement } from '@swagger-api/apidom-core';

const arrayElementMerge = (destination, source, options) => source;

const target = new ArrayElement([1, 2, 3]);
const source = new ArrayElement([3, 2, 1]);
const output = deepmerge(target, source, { arrayElementMerge });
// => ArrayElement([3, 2, 1]);
```

#### objectElementMerge

There are multiple ways to merge two ObjectElements, below are a few examples, but you can also create your own custom function.

Your `objectElementMerge` function will be called with three arguments: a `target` ObjectElement, the `source` ObjectElement,
and an `options` object.

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const objectElementMerge = (destination, source, options) => source;

const target = new ObjectElement({a: 1, b: 2});
const source = new ObjectElement({c: 3, d: 4});
const output = deepmerge(target ,source, { objectElementMerge });
// => ObjectElement({c: 3, d: 4});
```

#### isMergeableElement

By default, deepmerge clones every member from ObjectElement and ArrayElement.
You may not want this, if your ObjectElements are of special types,
and you want to copy the whole ObjectElement instead of just copying its member.

You can accomplish this by passing in a function for the `isMergeableElement` option.

```js
import { deepmerge, ObjectElement, isObjectElement } from '@swagger-api/apidom-core';

class CustomObjectElement extends ObjectElement {
  element = 'custom';
}
const instantiatedCustomObjectElement = new CustomObjectElement({ special: 'oh yeah' });

const target = new ObjectElement({
  someProperty: {
    cool: 'oh for sure',
  },
});
const source = new ObjectElement({
  someProperty: instantiatedCustomObjectElement,
});
const isMergeableElement = (element: Element) => isObjectElement(element) && !(element instanceof CustomObjectElement);

const output = deepmerge(target, source, {
  isMergeableElement,
});
// output.get('someProperty').get('cool'); // => undefined
// output.get('someProperty').get('special'); // => 'oh yeah'
// output.get('someProperty') instanceof CustomObjectElement // => true
```

#### customMerge

Specifies a function which can be used to override the default merge behavior for a member, based on the key name.
The `customMerge` function will be passed the key for each member, and should return the function which should
be used to merge the values for that member.
It may also return undefined, in which case the default merge behaviour will be used.

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const alex = new ObjectElement({
	name: {
		first: 'Alex',
		last: 'Alexson'
	},
	pets: ['Cat', 'Parrot']
});
const tony = new ObjectElement({
	name: {
		first: 'Tony',
		last: 'Tonison'
	},
	pets: ['Dog']
});

const mergeNames = (nameA: ObjectElement, nameB: ObjectElement) =>
  new StringElement(`${toValue(nameA.get('first'))} and ${toValue(nameB.get('first'))}`);
const customMerge = (key: Element) => (toValue(key) === 'name' ? mergeNames : undefined);

const output = deepmerge(alex, tony, { customMerge });
// output.get('name'); // => StrignElement('Alex and Tony')
// output.get('pets'); // => ArrayElement(['Cat', 'Parrot', 'Dog'])
```

#### customMetaMerge

Specifies a function which can be used to override the default metadata merge behavior.
The `customMetaMerge` function will be passed target and source metadata. If not specified,
the default behavior is to deep copy metadata from target to new merged element.

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const alex = new ObjectElement({ name: { first: 'Alex' } }, { metaKey: true });
const tony = new ObjectElement({ name: { first: 'Tony' } }, { metaKey: false });

const customMetaMerge = (targetMeta, sourceMeta) => deepmerge(targetMeta, sourceMeta);

const output = deepmerge(alex, tony, { customMetaMerge });
// output.meta.get('metaKey') // => BooleanElement(false)
```

#### customAttributesMerge

Specifies a function which can be used to override the default attributes merge behavior.
The `customAttributesMerge` function will be passed target and source attributes. If not specified,
the default behavior is to deep copy attributes from target to new merged element.

```js
import { deepmerge, ObjectElement } from '@swagger-api/apidom-core';

const alex = new ObjectElement({ name: { first: 'Alex' } }, undefined, { attributeKey: true });
const tony = new ObjectElement({ name: { first: 'Tony' } }, undefined, { attributeKey: false });

const customAttributesMerge = (targetMeta, sourceMeta) => deepmerge(targetMeta, sourceMeta);

const output = deepmerge(alex, tony, { customAttributesMerge });
// output.attributs.get('attributeKey') // => BooleanElement(false)
```

#### clone

Defaults to `true`.

If `clone` is false then child elements will be copied directly instead of being cloned.

---

## Refractors

Refractor is a special layer inside the base namespace that can transform JavaScript structures
into generic ApiDOM structures built from elements of this base namespace.

**Refracting JavaScript structures**:

```js
import { ObjectElement } from '@swagger-api/apidom-core';

const object = {
    title: 'my title',
    description: 'my description',
    version: '0.1.0',
};

ObjectElement.refract(object); // => ObjectElement({ title, description, version })
```

```js
import { CommentElement } from '@swagger-api/apidom-core';

const comment = 'this is comment';

CommentElement.refract(comment); // => CommentElement('this is comment')
```

### Refractor plugins

Refractors can accept plugins as a second argument of refract static method.

```js
import { ObjectElement, StringElement } from '@swagger-api/apidom-core';

const object = { a: 'b' };

const plugin = ({ predicates, namespace }) => ({
  name: 'plugin',
  pre() {
      console.dir('runs before traversal');
  },
  visitor: {
    ObjectElement(objectElement) {
      objectElement.getMember('a').value = new StringElement('c');
    },
  },
  post() {
      console.dir('runs after traversal');
  },
});

ObjectElement.refract(object, { plugins: [plugin] }); // => ObjectElement({ a = 'c' })
```
You can define as many plugins as needed to enhance the resulting namespaced ApiDOM structure.
If multiple plugins with the same visitor method are defined, they run in parallel (just like in Babel).

#### Element identity plugin

`apidom-core` package comes with `refractorPluginElementIdentity`. When used, this plugin will
assign unique ID to all elements in ApiDOM tree.

```js
import { refractorPluginElementIdentity, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = ObjectElement.refract({ a: 'b' }, {
  plugins: [
    refractorPluginElementIdentity(),
  ]
});

objectElement.id; // 8RaWF9
objectElement.getMember('a').key.id; // NdHHV7
objectElement.getMember('a').value.id; // rFGVFP
```

You can configure the plugin to generate unique IDs in the specific length:

```js
import { refractorPluginElementIdentity, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = ObjectElement.refract({ a: 'b' }, {
  plugins: [
    refractorPluginElementIdentity({ length: 36}),
  ]
});

objectElement.id; // OnReGGrO7fMd9ztacvGfwGbOdGKuOFLiQQ1W
objectElement.getMember('a').key.id; // BN6rHsmqI56SMQ1elshtbgRVECtEWNYS9lmd
objectElement.getMember('a').value.id; // Ki4tWmf9xw9Lwb8MxkXJq1uONmJrmhXifmsI
```

#### Semantic element identity plugin

`apidom-core` package comes with `refractorPluginSemanticElementIdentity`. When used, this plugin will
assign unique ID to all non-primitive elements in ApiDOM tree. Primitive elements include
`ObjectElement`, `ArrayElement`, `StringElement`, `BooleanElement`, `NullElement` and `NumberElement`.

```js
import { refractorPluginSemanticElementIdentity, ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

const infoElement = InfoElement.refract({ title: 'title' });
const objectElement = ObjectElement.refract({ a: 'b', info: infoElement }, {
  plugins: [
    refractorPluginSemanticElementIdentity(),
  ]
});

objectElement.id; // ''
objectElement.getMember('a').key.id; // ''
objectElement.getMember('a').value.id; // ''
objectElement.getMember('info').key.id; // ''
objectElement.getMember('info').value.id; // '8RaWF9'
```

You can configure the plugin to generate unique IDs in the specific length:

```js
import { refractorPluginSemanticElementIdentity, ObjectElement } from '@swagger-api/apidom-core';
import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-1';

const infoElement = InfoElement.refract({ title: 'title' });
const objectElement = ObjectElement.refract({ a: 'b', info: infoElement }, {
  plugins: [
    refractorPluginSemanticElementIdentity({ length: 36 }),
  ]
});

objectElement.id; // ''
objectElement.getMember('a').key.id; // ''
objectElement.getMember('a').value.id; // ''
objectElement.getMember('info').key.id; // ''
objectElement.getMember('info').value.id; // 'OnReGGrO7fMd9ztacvGfwGbOdGKuOFLiQQ1W'
```

---

## Traversal

`apidom-core` comes with its own traversal algorithm along with couple of convenient abstractions on top of it.

### visit

[visit](https://github.com/swagger-api/apidom/blob/main/packages/apidom-core/src/traversal/visitor.ts#L128) will walk through an AST using a depth first traversal, calling
the visitor's enter function at each node in the traversal, and calling the
leave function after visiting that node and all of its child nodes.

By returning different values from the enter and leave functions, the
behavior of the visitor can be altered, including skipping over a sub-tree of
the ApiDOM (by returning false), editing the ApiDOM by returning a value or null
to remove the value, or to stop the whole traversal by returning [BREAK](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts#L52).

When using `visit` to edit an ApiDOM, the original ApiDOM will not be modified, and
a new version of the ApiDOM with the changes applied will be returned from the
visit function.

```js
import { visit, ObjectElement, NumberElement } from '@swagger-api/apidom-core';

const visitor = {
    NumberElement(numberElement) {
        return new NumberElement(2);
    },
};
const element = new ObjectElement({ a: 1 });

const newElement = visit(element, visitor); // => ObjectElement<{a: 2}>
```

This function originally comes from [@swagger-api/apidom-ast package](https://github.com/swagger-api/apidom/blob/main/packages/apidom-ast/src/traversal/visitor.ts)
and is originally designed to work with [CST](https://en.wikipedia.org/wiki/Parse_tree). `apidom-core` package
imports it, specializes it to work with ApiDOM and re-export it.

All following algorithms are based on `visit` function.

### filter

Finds all elements matching the predicate.

```js
import { ObjectElement, filter, isNumberElement } from '@swagger-api/apidom-core'

const objElement = new ObjectElement({ a: 'b', c: 2 });

filter(isNumberElement, objElement); // => ArraySlice<[NumberElement<2>]>
```

### find

Find first element that satisfies the provided predicate.

```js
import { ObjectElement, find, isMemberElement } from '@swagger-api/apidom-core'

const objElement = new ObjectElement({ a: 'b', c: 2 });

find(isNumberElement, objElement); // => NumberElement<2>
```

### findAtOffset

ApiDOM nodes can be associated with source maps. This function finds the most inner node at the given offset.
If includeRightBound is set, also finds nodes that end at the given offset.

```js
import { findAtOffset } from '@swagger-api/apidom-core'

findAtOffset(3, elementWithSourceMaps); // => returns most inner node at offset 3
```

### reject

Complement of [filter](#filter).

```js
import { ArrayElement, reject, isNumberElement } from '@swagger-api/apidom-core'

const arrayElement = new ArrayElement([1, 'a']);

reject(isNumberElement, arrayElement); // => ArraySlice<[StringElement<'a'>]>
```

### some

Tests whether at least one element passes the predicate.

```js
import { ArrayElement, some, isNumberElement } from '@swagger-api/apidom-core'

const arrayElement = new ArrayElement([1, 'a']);

some(isNumberElement, arrayElement); // => true
```

### traverse

Executes the callback on this element and all descendants.

```js
import { ArrayElement, traverse } from '@swagger-api/apidom-core'

const arrayElement = new ArrayElement([1, 'a']);

traverse(console.dir, arrayElement); // => prints ArrayElement, NumberElement, StringElement in this order
```

The execution of the callback can be controlled further by providing a predicate.

```js
import { ArrayElement, traverse, isNumberElement } from '@swagger-api/apidom-core'

const arrayElement = new ArrayElement([1, 'a']);

traverse({ callback: console.dir, predicate: isNumberElement }, arrayElement); // => prints NumberElement<1>
```

### parents

Computes upwards edges from every child to its parent.

#### ObjectElement example

```js
import { parents, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = new ObjectElement({ key: 'value' });
const memberElement = objectElement.getMember('key');
const { key: keyElement, value: valueElement } = memberElement;

const parentEdges = parents(objectElement); // => WeakMap<ChildElement, ParentElement>

parentEdges.get(memberElement) === objectElement; // => true
parentEdges.get(keyElement) === memberElement; // => true
parentEdges.get(valueElement) === memberElement; // => true
```

#### ArrayElement example

```js
import { parents, ArrayElement, StringElement } from '@swagger-api/apidom-core';

const itemElement1 = new StringElement('item1');
const itemElement2 = new StringElement('item2');
const arrayElement = new ArrayElement([itemElement1, itemElement2]);

const parentEdges = parents(arrayElement); // => WeakMap<ChildElement, ParentElement>

parentEdges.get(itemElement1) === arrayElement; // => true
parentEdges.get(itemElement2) === arrayElement; // => true
```

---

## Transformers

Following functions transforms ApiDOM between its various forms. All transformers (except `toValue`) can accept
ApiDOM namespace instance as a second argument.

### from

Transforms data to an Element from a particular namespace.

From a [refracted string](https://github.com/refractproject/refract-spec) form:

```js
import { from } from '@swagger-api/apidom-core';

const refractedString = '{"element":"number","content":1}';

from(refractedString); // => NumberElement<1>
```

From a [refracted](https://github.com/refractproject/refract-spec) form:

```js
import { from } from '@swagger-api/apidom-core';

const refracted = { element: 'number', content: 1 };

from(refracted); // => NumberElement<1>
```

From a JavaScript form:

```js
import { from } from '@swagger-api/apidom-core';

const javascriptForm = 1;

from(javascriptForm); // => NumberElement<1>
```

### toValue

Transforms the ApiDOM into JavaScript POJO. This POJO would be the result of interpreting the ApiDOM
into JavaScript structure. This function can handle cycles in ApiDOM structure.

```js
import { toValue, ObjectElement } from '@swagger-api/apidom-core';

const objElement = new ObjectElement({ a: 'b' });

toValue(objElement); // => { a: 'b' }
```

### toJSON

Transforms the ApiDOM into JSON string.

```js
import { toJSON, ObjectElement } from '@swagger-api/apidom-core';

const objElement = new ObjectElement({ a: 'b' });

toJSON(objElement); // => '{"a":"b"}'
```

### toYAML

Transforms the ApiDOM into JSON string.

```js
import { toYAML, ObjectElement } from '@swagger-api/apidom-core';

const objElement = new ObjectElement({ a: 'b' });

toYAML(objElement);
/**
 * %YAML 1.2
 * ---
 *
 * "a": "b"
 */
```

### dehydrate

Creates a [refract representation](https://github.com/refractproject/refract-spec) of the an Element.

```js
import { dehyrate, NumberElement } from '@swagger-api/apidom-core';

const numberElement = new NumberElement(1);

dehyrate(numberElement); // => { element: 'number', content: 1 }
```

### S-Expression

Transforms ApiDOM into [symbolic expression](https://en.wikipedia.org/wiki/S-expression).

```js
import { sexprs, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = new ObjectElement({ a: 1 });

sexprs(objectElement);
// =>
// (ObjectElement
//   (MemberElement
//     (StringElement)
//     (NumberElement)))


```

### toString

Create a [refracted string](https://github.com/refractproject/refract-spec) representation of an Element.

```js
import { toString, NumberElement } from '@swagger-api/apidom-core';

const numberElement = new NumberElement(1);

toString(numberElement); // => '{"element":"number","content":1}'
```

## Cloning

Following functions provide mechanism for creating shallow and deep copies of ApiDOM elements.

### Shallow cloning

Creates shallow clone of ApiDOM element.

```js
import { cloneShallow, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = new ObjectElement({ a: 'b' });
const objectElementShallowClone = cloneShallow(objectElement);
```

### Deep cloning

Creates deep clone of ApiDOM Element.

```js
import { cloneDeep, ObjectElement } from '@swagger-api/apidom-core';

const objectElement = new ObjectElement({ a: 'b' });
const objectElementDeepClone = cloneDeep(objectElement);
```

