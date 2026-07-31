"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _filter = _interopRequireDefault(require("./filter.cjs"));
/**
 * Complement of filter.
 * @public
 */
const reject = (predicate, element) => {
  return (0, _filter.default)((0, _ramda.complement)(predicate), element);
};
var _default = exports.default = reject;