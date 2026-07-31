import { ObjectElement } from 'minim';
import { isObjectElement, isArrayElement } from "../predicates/index.mjs";
import { cloneDeep, cloneShallow } from "../clone/index.mjs";
import toValue from "../transformers/serializers/value/index.mjs";
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
export const emptyElement = element => {
  const meta = element.meta.length > 0 ? cloneDeep(element.meta) : undefined;
  const attributes = element.attributes.length > 0 ? cloneDeep(element.attributes) : undefined;

  // @ts-ignore
  return new element.constructor(undefined, meta, attributes);
};
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
    return targetMeta => cloneDeep(targetMeta);
  }
  return options.customMetaMerge;
};
const getAttributesMergeFunction = options => {
  if (typeof options.customAttributesMerge !== 'function') {
    return targetAttributes => cloneDeep(targetAttributes);
  }
  return options.customAttributesMerge;
};
const mergeArrayElement = (targetElement, sourceElement, options) => targetElement.concat(sourceElement)['fantasy-land/map'](item => cloneUnlessOtherwiseSpecified(item, options));
const mergeObjectElement = (targetElement, sourceElement, options) => {
  const destination = isObjectElement(targetElement) ? emptyElement(targetElement) : emptyElement(sourceElement);
  if (isObjectElement(targetElement)) {
    targetElement.forEach((value, key, member) => {
      const clonedMember = cloneShallow(member);
      clonedMember.value = cloneUnlessOtherwiseSpecified(value, options);
      destination.content.push(clonedMember);
    });
  }
  sourceElement.forEach((value, key, member) => {
    const keyValue = toValue(key);
    let clonedMember;
    if (isObjectElement(targetElement) && targetElement.hasKey(keyValue) && options.isMergeableElement(value)) {
      const targetValue = targetElement.get(keyValue);
      clonedMember = cloneShallow(member);
      clonedMember.value = getMergeFunction(key, options)(targetValue, value, options);
    } else {
      clonedMember = cloneShallow(member);
      clonedMember.value = cloneUnlessOtherwiseSpecified(value, options);
    }
    destination.remove(keyValue);
    destination.content.push(clonedMember);
  });
  return destination;
};
export const defaultOptions = {
  clone: true,
  isMergeableElement: element => isObjectElement(element) || isArrayElement(element),
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
  mergedOptions.isMergeableElement = (_mergedOptions$isMerg = mergedOptions.isMergeableElement) !== null && _mergedOptions$isMerg !== void 0 ? _mergedOptions$isMerg : defaultOptions.isMergeableElement;
  mergedOptions.arrayElementMerge = (_mergedOptions$arrayE = mergedOptions.arrayElementMerge) !== null && _mergedOptions$arrayE !== void 0 ? _mergedOptions$arrayE : defaultOptions.arrayElementMerge;
  mergedOptions.objectElementMerge = (_mergedOptions$object = mergedOptions.objectElementMerge) !== null && _mergedOptions$object !== void 0 ? _mergedOptions$object : defaultOptions.objectElementMerge;
  const sourceIsArrayElement = isArrayElement(sourceElement);
  const targetIsArrayElement = isArrayElement(targetElement);
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
    return new ObjectElement();
  }
  return list.reduce((target, source) => {
    return deepmerge(target, source, options);
  }, emptyElement(list[0]));
};
export default deepmerge;
/* eslint-enable @typescript-eslint/no-use-before-define */