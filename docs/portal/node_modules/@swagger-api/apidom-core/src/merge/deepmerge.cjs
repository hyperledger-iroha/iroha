"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.emptyElement = exports.defaultOptions = exports.default = void 0;
var _minim = require("minim");
var _index = require("../predicates/index.cjs");
var _index2 = require("../clone/index.cjs");
var _index3 = _interopRequireDefault(require("../transformers/serializers/value/index.cjs"));
/* eslint-disable @typescript-eslint/no-use-before-define */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

const emptyElement = element => {
  const meta = element.meta.length > 0 ? (0, _index2.cloneDeep)(element.meta) : undefined;
  const attributes = element.attributes.length > 0 ? (0, _index2.cloneDeep)(element.attributes) : undefined;

  // @ts-ignore
  return new element.constructor(undefined, meta, attributes);
};
exports.emptyElement = emptyElement;
const cloneUnlessOtherwiseSpecified = (element, options) => options.clone && options.isMergeableElement(element) ? deepmerge(emptyElement(element), element, options) : element;
const getMergeFunction = (keyElement, options) => {
  if (typeof options.customMerge !== 'function') {
    return deepmerge;
  }
  const customMerge = options.customMerge(keyElement, options);
  return typeof customMerge === 'function' ? customMerge : deepmerge;
};
const getMetaMergeFunction = options => {
  if (typeof options.customMetaMerge !== 'function') {
    return targetMeta => (0, _index2.cloneDeep)(targetMeta);
  }
  return options.customMetaMerge;
};
const getAttributesMergeFunction = options => {
  if (typeof options.customAttributesMerge !== 'function') {
    return targetAttributes => (0, _index2.cloneDeep)(targetAttributes);
  }
  return options.customAttributesMerge;
};
const mergeArrayElement = (targetElement, sourceElement, options) => targetElement.concat(sourceElement)['fantasy-land/map'](item => cloneUnlessOtherwiseSpecified(item, options));
const mergeObjectElement = (targetElement, sourceElement, options) => {
  const destination = (0, _index.isObjectElement)(targetElement) ? emptyElement(targetElement) : emptyElement(sourceElement);
  if ((0, _index.isObjectElement)(targetElement)) {
    targetElement.forEach((value, key, member) => {
      const clonedMember = (0, _index2.cloneShallow)(member);
      clonedMember.value = cloneUnlessOtherwiseSpecified(value, options);
      destination.content.push(clonedMember);
    });
  }
  sourceElement.forEach((value, key, member) => {
    const keyValue = (0, _index3.default)(key);
    let clonedMember;
    if ((0, _index.isObjectElement)(targetElement) && targetElement.hasKey(keyValue) && options.isMergeableElement(value)) {
      const targetValue = targetElement.get(keyValue);
      clonedMember = (0, _index2.cloneShallow)(member);
      clonedMember.value = getMergeFunction(key, options)(targetValue, value, options);
    } else {
      clonedMember = (0, _index2.cloneShallow)(member);
      clonedMember.value = cloneUnlessOtherwiseSpecified(value, options);
    }
    destination.remove(keyValue);
    destination.content.push(clonedMember);
  });
  return destination;
};
const defaultOptions = exports.defaultOptions = {
  clone: true,
  isMergeableElement: element => (0, _index.isObjectElement)(element) || (0, _index.isArrayElement)(element),
  arrayElementMerge: mergeArrayElement,
  objectElementMerge: mergeObjectElement,
  customMerge: undefined,
  customMetaMerge: undefined,
  customAttributesMerge: undefined
};

/**
 * @public
 */
const deepmerge = (targetElement, sourceElement, options) => {
  var _mergedOptions$isMerg, _mergedOptions$arrayE, _mergedOptions$object;
  const mergedOptions = {
    ...defaultOptions,
    ...options
  };
  mergedOptions.isMergeableElement = (_mergedOptions$isMerg = mergedOptions.isMergeableElement) != null ? _mergedOptions$isMerg : defaultOptions.isMergeableElement;
  mergedOptions.arrayElementMerge = (_mergedOptions$arrayE = mergedOptions.arrayElementMerge) != null ? _mergedOptions$arrayE : defaultOptions.arrayElementMerge;
  mergedOptions.objectElementMerge = (_mergedOptions$object = mergedOptions.objectElementMerge) != null ? _mergedOptions$object : defaultOptions.objectElementMerge;
  const sourceIsArrayElement = (0, _index.isArrayElement)(sourceElement);
  const targetIsArrayElement = (0, _index.isArrayElement)(targetElement);
  const sourceAndTargetTypesMatch = sourceIsArrayElement === targetIsArrayElement;
  if (!sourceAndTargetTypesMatch) {
    return cloneUnlessOtherwiseSpecified(sourceElement, mergedOptions);
  }

  // merging two elements
  const mergedElement = sourceIsArrayElement && typeof mergedOptions.arrayElementMerge === 'function' ? mergedOptions.arrayElementMerge(targetElement, sourceElement, mergedOptions) : mergedOptions.objectElementMerge(targetElement, sourceElement, mergedOptions);

  // merging meta & attributes
  mergedElement.meta = getMetaMergeFunction(mergedOptions)(targetElement.meta, sourceElement.meta);
  mergedElement.attributes = getAttributesMergeFunction(mergedOptions)(targetElement.attributes, sourceElement.attributes);
  return mergedElement;
};
deepmerge.all = (list, options) => {
  if (!Array.isArray(list)) {
    throw new TypeError('First argument of deepmerge should be an array.');
  }
  if (list.length === 0) {
    return new _minim.ObjectElement();
  }
  return list.reduce((target, source) => {
    return deepmerge(target, source, options);
  }, emptyElement(list[0]));
};
var _default = exports.default = deepmerge;
/* eslint-enable @typescript-eslint/no-use-before-define */