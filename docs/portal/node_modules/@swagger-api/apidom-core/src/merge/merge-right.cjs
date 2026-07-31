"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _minim = require("minim");
var _deepmerge = _interopRequireWildcard(require("./deepmerge.cjs"));
/**
 * @public
 */

/**
 * @public
 */
const mergeRight = (targetElement, sourceElement, options) => {
  const mergedOptions = {
    ..._deepmerge.defaultOptions,
    ...options,
    customMerge: () => (target, source) => source,
    clone: false
  };
  return (0, _deepmerge.default)(targetElement, sourceElement, mergedOptions);
};
mergeRight.all = (list, options) => {
  if (!Array.isArray(list)) {
    throw new TypeError('First argument of mergeRight should be an array.');
  }
  if (list.length === 0) {
    return new _minim.ObjectElement();
  }
  return list.reduce((target, source) => {
    return mergeRight(target, source, options);
  }, (0, _deepmerge.emptyElement)(list[0]));
};
var _default = exports.default = mergeRight;