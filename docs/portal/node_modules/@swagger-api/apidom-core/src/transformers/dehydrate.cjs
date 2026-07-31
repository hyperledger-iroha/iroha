"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _namespace = _interopRequireDefault(require("../namespace.cjs"));
/**
 * Creates a refract representation of an Element.
 * https://github.com/refractproject/refract-spec
 * @public
 */
const dehydrate = (element, namespace = _namespace.default) => {
  return namespace.toRefract(element);
};
var _default = exports.default = dehydrate;