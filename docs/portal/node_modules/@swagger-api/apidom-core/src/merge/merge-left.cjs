"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _mergeRight = _interopRequireDefault(require("./merge-right.cjs"));
/**
 * @public
 */
const mergeLeft = (...[sourceElement, targetElement, options]) => {
  return (0, _mergeRight.default)(targetElement, sourceElement, options);
};
mergeLeft.all = (...[list, options]) => {
  return _mergeRight.default.all([...list].reverse(), options);
};
var _default = exports.default = mergeLeft;