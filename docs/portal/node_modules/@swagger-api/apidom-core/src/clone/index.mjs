import { ArraySlice, ObjectSlice, KeyValuePair } from 'minim';
import { hasElementSourceMap, isElement } from "../predicates/index.mjs";
import DeepCloneError from "./errors/DeepCloneError.mjs";
import ShallowCloneError from "./errors/ShallowCloneError.mjs";
import { assignSourceMap } from "../util.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
export const cloneDeep = (value, options = {}) => {
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
  if (value instanceof KeyValuePair) {
    const {
      key,
      value: val
    } = value;
    const keyCopy = isElement(key) ? cloneDeep(key, passThroughOptions) : key;
    const valueCopy = isElement(val) ? cloneDeep(val, passThroughOptions) : val;
    const copy = new KeyValuePair(keyCopy, valueCopy);
    visited.set(value, copy);
    return copy;
  }
  if (value instanceof ObjectSlice) {
    const mapper = element => cloneDeep(element, passThroughOptions);
    const items = [...value].map(mapper);
    const copy = new ObjectSlice(items);
    visited.set(value, copy);
    return copy;
  }
  if (value instanceof ArraySlice) {
    const mapper = element => cloneDeep(element, passThroughOptions);
    const items = [...value].map(mapper);
    const copy = new ArraySlice(items);
    visited.set(value, copy);
    return copy;
  }
  if (isElement(value)) {
    const copy = cloneShallow(value); // eslint-disable-line @typescript-eslint/no-use-before-define

    visited.set(value, copy);
    if (value.content) {
      if (isElement(value.content)) {
        copy.content = cloneDeep(value.content, passThroughOptions);
      } else if (value.content instanceof KeyValuePair) {
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
  throw new DeepCloneError("Value provided to cloneDeep function couldn't be cloned", {
    value
  });
};
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
  return new KeyValuePair(key, value);
};
const cloneShallowArraySlice = arraySlice => {
  const items = [...arraySlice];
  return new ArraySlice(items);
};
const cloneShallowObjectSlice = objectSlice => {
  const items = [...objectSlice];
  return new ObjectSlice(items);
};

/* eslint-disable no-underscore-dangle */
const cloneShallowElement = element => {
  // @ts-ignore
  const copy = new element.constructor();
  copy.element = element.element;
  if (hasElementSourceMap(element)) {
    assignSourceMap(copy, element);
  }
  if (element.meta.length > 0) {
    copy._meta = cloneDeep(element.meta);
  }
  if (element.attributes.length > 0) {
    copy._attributes = cloneDeep(element.attributes);
  }
  if (isElement(element.content)) {
    const content = element.content;
    copy.content = cloneShallowElement(content);
  } else if (Array.isArray(element.content)) {
    copy.content = [...element.content];
  } else if (element.content instanceof KeyValuePair) {
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
export const cloneShallow = value => {
  if (value instanceof KeyValuePair) {
    return cloneShallowKeyValuePair(value);
  }
  if (value instanceof ObjectSlice) {
    return cloneShallowObjectSlice(value);
  }
  if (value instanceof ArraySlice) {
    return cloneShallowArraySlice(value);
  }
  if (isElement(value)) {
    return cloneShallowElement(value);
  }
  throw new ShallowCloneError("Value provided to cloneShallow function couldn't be cloned", {
    value
  });
};
cloneShallow.safe = value => {
  try {
    return cloneShallow(value);
  } catch {
    return value;
  }
};