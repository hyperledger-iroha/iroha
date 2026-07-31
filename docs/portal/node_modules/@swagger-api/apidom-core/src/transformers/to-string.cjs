"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _namespace = _interopRequireDefault(require("../namespace.cjs"));
var _dehydrate = _interopRequireDefault(require("./dehydrate.cjs"));
/**
 * Create a refracted string representation of an Element.
 * @public
 */
const toString = (element, namespace = _namespace.default) => {
  const refractStructure = (0, _dehydrate.default)(element, namespace);
  return JSON.stringify(refractStructure);
};
var _default = exports.default = toString;