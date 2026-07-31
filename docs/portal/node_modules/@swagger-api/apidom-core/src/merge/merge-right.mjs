import { ObjectElement } from 'minim';
import deepmerge, { defaultOptions as defaultDeepmergeOptions, emptyElement } from "./deepmerge.mjs";
/**
 * @public
 */
/**
 * @public
 */
const mergeRight = (targetElement, sourceElement, options) => {
  const mergedOptions = {
    ...defaultDeepmergeOptions,
    ...options,
    customMerge: () => (target, source) => source,
    clone: false
  };
  return deepmerge(targetElement, sourceElement, mergedOptions);
};
mergeRight.all = (list, options) => {
  if (!Array.isArray(list)) {
    throw new TypeError('First argument of mergeRight should be an array.');
  }
  if (list.length === 0) {
    return new ObjectElement();
  }
  return list.reduce((target, source) => {
    return mergeRight(target, source, options);
  }, emptyElement(list[0]));
};
export default mergeRight;