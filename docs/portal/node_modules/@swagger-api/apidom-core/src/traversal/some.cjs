"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _find = _interopRequireDefault(require("./find.cjs"));
/**
 * Tests whether at least one element passes the predicate.
 * @public
 */
const some = (predicate, element) => {
  return (0, _ramdaAdjunct.isNotUndefined)((0, _find.default)(predicate, element));
};
var _default = exports.default = some;