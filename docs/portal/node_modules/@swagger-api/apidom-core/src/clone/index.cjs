"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.cloneShallow = exports.cloneDeep = void 0;
var _minim = require("minim");
var _index = require("../predicates/index.cjs");
var _DeepCloneError = _interopRequireDefault(require("./errors/DeepCloneError.cjs"));
var _ShallowCloneError = _interopRequireDefault(require("./errors/ShallowCloneError.cjs"));
var _util = require("../util.cjs");
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
const cloneDeep = (value, options = {}) => {
  const {
    visited = new WeakMap()
  } = options;
  const passThroughOptions = {
    ...options,
    visited
  };

  // detect cycle and return memoized value
  if (visited.has(value)) {
    return visited.get(value);
  }
  if (value instanceof _minim.KeyValuePair) {
    const {
      key,
      value: val
    } = value;
    const keyCopy = (0, _index.isElement)(key) ? cloneDeep(key, passThroughOptions) : key;
    const valueCopy = (0, _index.isElement)(val) ? cloneDeep(val, passThroughOptions) : val;
    const copy = new _minim.KeyValuePair(keyCopy, valueCopy);
    visited.set(value, copy);
    return copy;
  }
  if (value instanceof _minim.ObjectSlice) {
    const mapper = element => cloneDeep(element, passThroughOptions);
    const items = [...value].map(mapper);
    const copy = new _minim.ObjectSlice(items);
    visited.set(value, copy);
    return copy;
  }
  if (value instanceof _minim.ArraySlice) {
    const mapper = element => cloneDeep(element, passThroughOptions);
    const items = [...value].map(mapper);
    const copy = new _minim.ArraySlice(items);
    visited.set(value, copy);
    return copy;
  }
  if ((0, _index.isElement)(value)) {
    const copy = cloneShallow(value); // eslint-disable-line @typescript-eslint/no-use-before-define

    visited.set(value, copy);
    if (value.content) {
      if ((0, _index.isElement)(value.content)) {
        copy.content = cloneDeep(value.content, passThroughOptions);
      } else if (value.content instanceof _minim.KeyValuePair) {
        copy.content = cloneDeep(value.content, passThroughOptions);
      } else if (Array.isArray(value.content)) {
        const mapper = element => cloneDeep(element, passThroughOptions);
        copy.content = value.content.map(mapper);
      } else {
        copy.content = value.content;
      }
    } else {
      copy.content = value.content;
    }
    return copy;
  }
  throw new _DeepCloneError.default("Value provided to cloneDeep function couldn't be cloned", {
    value
  });
};
exports.cloneDeep = cloneDeep;
cloneDeep.safe = value => {
  try {
    return cloneDeep(value);
  } catch {
    return value;
  }
};
const cloneShallowKeyValuePair = keyValuePair => {
  const {
    key,
    value
  } = keyValuePair;
  return new _minim.KeyValuePair(key, value);
};
const cloneShallowArraySlice = arraySlice => {
  const items = [...arraySlice];
  return new _minim.ArraySlice(items);
};
const cloneShallowObjectSlice = objectSlice => {
  const items = [...objectSlice];
  return new _minim.ObjectSlice(items);
};

/* eslint-disable no-underscore-dangle */
const cloneShallowElement = element => {
  // @ts-ignore
  const copy = new element.constructor();
  copy.element = element.element;
  if ((0, _index.hasElementSourceMap)(element)) {
    (0, _util.assignSourceMap)(copy, element);
  }
  if (element.meta.length > 0) {
    copy._meta = cloneDeep(element.meta);
  }
  if (element.attributes.length > 0) {
    copy._attributes = cloneDeep(element.attributes);
  }
  if ((0, _index.isElement)(element.content)) {
    const content = element.content;
    copy.content = cloneShallowElement(content);
  } else if (Array.isArray(element.content)) {
    copy.content = [...element.content];
  } else if (element.content instanceof _minim.KeyValuePair) {
    copy.content = cloneShallowKeyValuePair(element.content);
  } else {
    copy.content = element.content;
  }
  return copy;
};
/* eslint-enable */

/**
 * @public
 */
const cloneShallow = value => {
  if (value instanceof _minim.KeyValuePair) {
    return cloneShallowKeyValuePair(value);
  }
  if (value instanceof _minim.ObjectSlice) {
    return cloneShallowObjectSlice(value);
  }
  if (value instanceof _minim.ArraySlice) {
    return cloneShallowArraySlice(value);
  }
  if ((0, _index.isElement)(value)) {
    return cloneShallowElement(value);
  }
  throw new _ShallowCloneError.default("Value provided to cloneShallow function couldn't be cloned", {
    value
  });
};
exports.cloneShallow = cloneShallow;
cloneShallow.safe = value => {
  try {
    return cloneShallow(value);
  } catch {
    return value;
  }
};