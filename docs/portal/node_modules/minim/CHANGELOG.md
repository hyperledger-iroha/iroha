# Minim Changelog

## 0.23.8 (2020-06-12)

### Enhancements

- `ArrayElement`'s `contains` method has been renamed to `includes` to be
  consistent with `Array.includes`. `ArrayElement.contains` has been
  deprecated, and remains for compatibility.

### Bug Fixes

- Prevent throwing an error when calling `toValue()` on an element with a key
  value pair which does not have a value.

## 0.23.7 (2020-04-27)

### Bug Fixes

- Prevents the JSON serializer from serializing an empty object (`{}`) under
  meta and attributes under the case where none of the meta or attribute
  member's have a value. This prevents `{}` from being present under meta or
  attributes when setting a member with an undefined key.

## 0.23.6 (2019-09-10)

### Bug Fixes

- Fixes a JSON 0.6 serialisation bug where httpRequest and similar array-based
  elements with undefined content would be serialised with undefined content
  instead of an empty array as content.

## 0.23.5 (2019-07-02)

This release brings some performance improvements, namely to serialising with
the JSON serialisers.

## 0.23.4 (2019-06-11)

### Bug Fixes

- Fixes serialisation of default values in enumerations in 
  Refract JSON 0.6 serialisation.

## 0.23.3 (2019-04-06)

### Enhancements

- Added support for IE11 in the included web distribution of minim
  (`dist/minim.js`).

## 0.23.2 (2019-03-15)

### Bug Fixes

- Fixes serialisation of array and object sample values in enumerations in
  Refract JSON 0.6 serialisation.

## 0.23.1 (2019-02-25)

### Bug Fixes

- Restores documentation coverage for all elements, some was unintentionally
  removed in 0.23.0.

## 0.23.0 (2019-02-22)

### Breaking

- Support for Node 4 has been removed. Minim now supports Node >= 6.
- Minim no longer uses [uptown](http://github.com/smizell/uptown) and thus the
  `extend` API has been removed.

### Enhancements

- Calling `.freeze()` on a frozen element is now supported. Previously you may
  see an error thrown while freeze was trying to attach parents to any child
  elements.

## 0.22.1 (2018-12-10)

### Bug Fixes

- Fixes serialising an element with an undefined meta or attributes value. For
  example if a meta value (`id`) was set to `undefined`, then it should not be
  serialised. Previously the serialiser would throw an exception that
  undefined was not an element.

## 0.22.0

### Enhancements

- `ArrayElement` now conforms to parts of the [Fantasy
  Land](https://github.com/fantasyland/fantasy-land) 3.5 specification.
  `Functor`, `Semigroup`, `Monoid`, `Filterable`, `Chain`, and `Foldable` are
  now supported.

## 0.21.1

### Bug Fixes

- Empty parseResult and link arrays are serialised in JSON 06 Serialiser, a
  regression of 0.21.0 caused these to not be serialised.

## 0.21.0

### Breaking

- Minim no longer supports importing files directly from the minim package.
  Importing the JSON 0.6 serialiser via
  `require('minim/lib/serialisers/json-0.6')` is not supported, it is now
  recommended to import `JSON06Serialiser` and other APIs from minim directly.

    ```js
    const { JSON06Serialiser } = require('minim');
    ```

- `flatMap` in `ArraySlice` no longer removes empty items. Instead `flatMap` is
  aligned with
  [`Array.flatMap`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/flatMap)
  which first maps each element using a mapping function, then flattens the
  result into a new array.

  Existing `flatMap` behaviour is now available under the method `compactMap`.

### Enhancements

- Object Element can now be created with an array of member elements.

- You can now create an element from an ArraySlice or ObjectSlice, for example,
  passing the result of a `filter` operation into a new element.

  ```
  const numbers = new ArrayElement([1, 2, 3, 4])
  new ArrayElement(numbers.filter((e) => e.toValue() % 2))
  ```

- Adds `compactMap` functionality to Array and Object elements allowing you to
  returns an array containing the truthy results of calling the given
  transformation with each element of this sequence.

- Added `flatMap` to `ArrayElement`.

### Bug Fixes

- The default content value of an element is undefined. Whereas before the
  default value was `null`.

- Setting the `content` property on an Element now behaves the same as passing
  content in to the constructor. For example, the following two elements are
  identical:

  ```js
  new ArrayElement([1])

  const element = new ArrayElement()
  element.content = [1]
  ```

  Passing `[1]` to an `ArrayElement` constructor would produce an array of
  number elements, whereas setting the content to `[1]` resulted in setting the
  content to be an array of non-elements which is invalid.

- The serialisation of the `variable` attribute in the JSON 0.6 serialisation
  is updated to reflect API Elements 1.0. The `variable` attribute is now
  present on a member element instead of the key of a member element.

- Empty arrays are no longer serialised in JSON 06 Serialiser.

## 0.20.7

### Bug Fixes

- Fixes a regression from 0.20.6 where `metadata` became an `ObjectElement`
  instead of `ArrayElement` as it was in the past.

## 0.20.6

### Bug Fixes

- JSON 0.6 deserialiser will now correct deserialise an API Categories `meta`
  attribute into `metadata`.

- JSON Serialisers will now use elements from the given namespace during
  serialisation checks and deserialisation.

## 0.20.5

### Bug Fixes

- JSON 0.6 enum serialisation will now remove `fixed` typeAttributes which are
  now present in API Elements 1.0 enumerations. These are removed for
  consistent serialisation of the 0.6 serialiser.

## 0.20.4

- Further performance improvements have been made to JSON Serialisation. The
  serialiser can now deserialise deep structures substantially faster.

## 0.20.3

### Enhancements

- Minim NPM package now contains a browser distribution in `dist/minim.js`.
- Performance improvements have been made to JSON Serialisation. The serialiser
  can now serialise deep structures a little faster.

## 0.20.2

### Bug Fixes

- The JSON 0.6 serialiser will now serialise empty content arrays. A regression
  caused in 0.20.1 because of the logic was applied to both Refract JSON 1.0
  and 0.6 serialisers.

## 0.20.1

### Bug Fixes

- Prevent de-serialising `undefined` if the default element's content is not
  null.
- No longer serialise an empty array in the JSON serialisers, instead the
  content can be removed for consistency with other tools.

## 0.20.0

### Enhancements

- Adds a `reject` method to `ArrayElement`, `ObjectElement`, `ArraySlice`,
  and `ObjectSlice` which complements the `filter` method providing the ability
  to exclude vs filter matched elements.

### Breaking

- The Refract JSON 0.6 serialiser will de-serialise enum elements into the form
  in the API Elements 1.0 specification. This is a breaking change on the
  layout of the enum. Default and sample values will now be an `enum` element
  themselves.

### Bug Fixes

- JSON deserialisers will now prevent overriding default element content
  values with undefined. This could cause problems where internal state of
  array or object element would have undefined as content and thus cause other
  Element methods to later fail such as `toValue` or `get`.

## 0.19.2

### Enhancements

- ArraySlice now provides a `find` method allowing you to find the first
  element satisfying the given value.
- ArraySlice.filter now accepts element names or element classes to filter.
- ArraySlice now provides `flatMap` allowing you to map and then flatten the
  results.

### Bug Fixes

- Accessing lazy meta accessors on frozen elements such as `title` will now
  return a frozen default value. Previously this would raise an exception
  trying to mutate the element.

## 0.19.1

### Enhancements

- Serialisers will now throw TypeError with straight forward messages when you
  try to serialise a non-element type.

### Bug Fixes

- While accessing meta or attributes of a frozen element that does not contain
  meta or attributes, an exception was raised because these accessors would
  lazy load and attempt to mutate the element.

  These accessors will now return an empty frozen `ObjectElement` in these
  cases now to prevent mutation.
- Fixes JSON 0.6 Deserialiser to correct deserialise enum elements.
    - When multiple sample values were present additional values were being discarded.
    - Deserialised enum content contained duplicate enumeration values.

## 0.19.0

### Breaking

- Updated enum serialization/deserialization in the JSON 0.6 serializer to match
  https://github.com/apiaryio/api-elements/pull/28
- `Element.children` and `Element.recursiveChildren` now return `ArraySlice`
  instead of an `ArrayElement`.
- `ArrayElement.filter` and `ArrayElement.find*` now return `ArraySlice`
  instead of an `ArrayElement`.
- The `first`, `second` and `last` methods on `ArrayElement` are now properties
  instead of methods.
- `ObjectElement.filter` now returns an `ObjectSlice` instead of an
  `ObjectElement`.
- When providing multiple element names to `Element.findRecursive` you must
  call `freeze` on the element beforehand so that the element has access to the
  parent of the element.

### Enhancements

- Introduced JSDoc documentation to public interfaces
- `Element` now contains a `freeze` method to freeze and prevent an element
  from being mutated, this also adds a parent property on all child elements.

### Bug Fixes

- Handle serializing key-value pair without value
- Deserialize `dataStructure` containing an array correctly

## 0.18.1

### Bug Fixes

- Prevent JSON Serialisers from throwing exception when serialising a key value
  pair without any value.

## 0.18.0

### Breaking

- JSON Serialisation now follows the JSON Refract serialisation rules defined at
  https://github.com/refractproject/refract-spec/blob/master/formats/json-refract.md.

  Existing serialiser is available during a transition period to aid migration
  to the new format.

    ```js
    const JSONSerialiser = require('minim/serialisers/json-0.6');
    const serialiser = new JSONSerialiser();
    const element = serialiser.deserialise('Hello');
    serialiser.serialise(element);
    ```

### Enhancements

- ArrayElement high-order functions, `map`, `filter` and `forEach` now accept
  `thisArg` like the equivalent functionality in `Array`.

## 0.17.1 (2016-07-29)

### Bug Fixes

- Initialising an Element with given meta or attributes as ObjectElement is now
  supported.
- When converting JavaScript values to Refract, objects are now supported.
- Adds a special case to serialise sourceMap elements as values.

## 0.17.0 (2017-06-16)

### Breaking

- `Element.toRefract()` and `Element.fromRefract()` have been removed. JSON
  Serialisation is now decoupled from the Element model. A minim namespace
  provides a convenience `toRefract(element)` and `fromRefract(object)`
  methods.

- `ArrayElement` `children` method has been replaced by a `children` property
  on all elements. You may now chain children in conjunction with `filter` to
  get the existing behaviour.

  Before:

  ```js
  const numbers = doc.children((element) => element.element == 'number');
  ```

  After:

  ```js
  const numbers = doc.children.filter((element) => element.element == 'number');
  ```

  *OR*

  ```js
  const numbers = doc.children.findByElement('number');
  ```

- `BaseElement` has been renamed to `Element`.

- Embedded Refract support has been removed.

### Enhancements

- All elements now contain a `children` and `recursiveChildren` properties that
  return an ArrayElement of the respective children elements.
- JSON Serialiser will no longer serialise empty `meta` and `attributes` into
  JSON objects.
- Minim now contains a `RefElement`.
- Element now contains a `toRef()` function to create a ref element referencing
  the element.

## 0.16.0 (2017-05-04)

### Breaking

- Node 0.10 and 0.12 are no longer supported.
- Elements `name` property was removed. There is no longer a name property in
  Refract specification.

### Enhancements

- Elements now provide a `findRecursive` method allowing you to recursively
  find matching elements.
- Added function for remove key in an Object element and Array element

#### Array Element

- New `isEmpty` convenience property for determining if an array is empty.

## 0.15.0 (2017-04-03)

- Getters of link element will now return an element
- Meta convenience methods will now return an element

## 0.14.2 (2016-08-19)

- Update Lodash version

## 0.14.1 (2016-08-17)

- Update Uptown to 0.4.1

## 0.14.0 (2016-04-28)

- **BREAKING** The public interface of the `minim` module has changed significantly. List of changes:

  - Removed `toCompactRefract` and `fromCompactRefract`
  - Improved the default refract serialization such that when an element in `attributes` has its own metadata or attributes defined then it will now be refracted when calling `toRefract`

## 0.13.0 (2015-12-03)

- Added support for hyperlinks per [RFC 0008](https://github.com/refractproject/rfcs/blob/b6e390f7bbc960808ba053e172cccd9e4a81a04a/text/0008-add-hyperlinks.md)
- Upgraded Lodash to 3.10.1
- Refract elements will be automatically parsed when found in arrays in `meta`

## 0.12.3 (2015-11-30)

- When an element in `meta` has its own metadata or attributes defined then it will now be refracted when calling `toRefract` or `toCompactRefract`.
- When loading from refract or compact refract, if an item in `meta` looks like an element it will be loaded as such. This may cause false positives.

## 0.12.2 (2015-11-24)

- Fix a bug related to setting the default key names that should be treated as refracted elements in element attributes. This is now accomplished via the namespace: `namespace._elementAttributeKeys.push('my-value');`. This fixes bugs related to overwriting the `namespace.BaseElement`.

## 0.12.1 (2015-11-24)

- Fix a bug when loading refracted attributes from compact refract.

## 0.12.0 (2015-11-23)

- Provide a way for elements to mark attributes as unrefracted arrays of
  refracted elements. Subclassed elements can push onto the
  `_attributeElementArrayKeys` property to use this feature. **Note**: in the
  future this feature may go away.
- Allow `load` to be used for plugins where a namespace is not being used
- Add an `elements` property to the `Namespace` class which returns an object of PascalCased element name keys to registered element class values. This allows for ES6 use cases like:

  ```js
  const {StringElement, ArrayElement, ObjectElement} = namespace.elements;
  ```

- Add functionality for Embedded Refract

## 0.11.0 (2015-09-07)

### Breaking

The public interface of the `minim` module has changed significantly. List of changes:

  - `ElementRegistry` has been renamed to `Namespace`.
  - `minim` has only one public method, called `namespace`, which creates a new `Namespace` instance.
  - `minim.convertToElement` is now `namespace.toElement`
  - `minim.convertFromRefract` is now `namespace.fromRefract`
  - `minim.convertFromCompactRefract` is now `namespace.fromCompactRefract`
  - `minim.*Element` are removed (except for `namespace.BaseElement`). These should be accessed via `namespace.getElementClass('name')` now.
  - The `Namespace` has a new method `use` which loads a plugin namespace and is chainable, e.g. `namespace.use(plugin1).use(plugin2)`.
  - A `Namespace` can be initialized without any default elements by passing an options object with `noDefault` set to `false` to the constructor. They can be initialized later via the `useDefault` method.

  Before:

  ```js
  var minim = require('minim');
  minim.convertToElement([1, 2, 3]);
  ```

  After:

  ```js
  var minim = require('minim');
  var namespace = minim.namespace();
  namespace.toElement([1, 2, 3]);
  ```

- Add a `.toValue()` method to member elements which returns a hash with the key
   and value and their respective values.

## 0.10.0 (2015-08-18)

- Rename the `class` metadata property to `classes`. The convenience property
  is also now called `classes`, e.g. `element.classes.contains('abc')`.

## 0.9.0 (2015-07-28)

- Allow the iterator protocol to be used with arrays and objects if the runtime
  supports it. This enables using `for ... of` loops on elements as well as
  rest operators, destructuring, `yield*`, etc.
- Convenience properties for simple types now return the value result. Instead
  of `element.title.toValue()` you now use `element.title`.
- Add array indexes to `#forEach`.
- Add a `#clone` method.
- Add a `#reduce` method.
- Fix a serialization bug when initializing using falsey values
  (`null`, `0`, `false`).

## 0.8.0 (2015-07-09)

- Allow `#set` to take an object for Object Elements
- Convert `meta` to be Minim Object Elements
- Convert `attributes` to be Minim Object Elements
- Sync class and method names with Refract 0.2.0 spec
- Add convenience methods for `meta` attributes, such as `id` or `class`
- Add finder functions, such as `findByElement` and `findByClass`
- Upgrade to use Uptown 0.4.0
- Organize code
