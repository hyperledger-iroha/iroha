const Element = require('./primitives/Element');
const NullElement = require('./primitives/NullElement');
const StringElement = require('./primitives/StringElement');
const NumberElement = require('./primitives/NumberElement');
const BooleanElement = require('./primitives/BooleanElement');
const ArrayElement = require('./primitives/ArrayElement');
const MemberElement = require('./primitives/MemberElement');
const ObjectElement = require('./primitives/ObjectElement');
const LinkElement = require('./elements/LinkElement');
const RefElement = require('./elements/RefElement');

const ArraySlice = require('./ArraySlice');
const ObjectSlice = require('./ObjectSlice');

const KeyValuePair = require('./KeyValuePair');

/**
 * Refracts a JSON type to minim elements
 * @param value
 * @returns {Element}
 */
function refract(value) {
  if (value instanceof Element) {
    return value;
  }

  if (typeof value === 'string') {
    return new StringElement(value);
  }

  if (typeof value === 'number') {
    return new NumberElement(value);
  }

  if (typeof value === 'boolean') {
    return new BooleanElement(value);
  }

  if (value === null) {
    return new NullElement();
  }

  if (Array.isArray(value)) {
    return new ArrayElement(value.map(refract));
  }

  if (typeof value === 'object') {
    const element = new ObjectElement(value);
    return element;
  }

  return value;
}

Element.prototype.ObjectElement = ObjectElement;
Element.prototype.RefElement = RefElement;
Element.prototype.MemberElement = MemberElement;

Element.prototype.refract = refract;
ArraySlice.prototype.refract = refract;

/**
 * Contains all of the element classes, and related structures and methods
 * for handling with element instances.
 */
module.exports = {
  Element,
  NullElement,
  StringElement,
  NumberElement,
  BooleanElement,
  ArrayElement,
  MemberElement,
  ObjectElement,
  LinkElement,
  RefElement,

  refract,

  ArraySlice,
  ObjectSlice,
  KeyValuePair,
};
