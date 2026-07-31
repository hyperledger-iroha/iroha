"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _toInteger = _interopRequireDefault(require("./toInteger32.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is a signed 32 bit integer.
 *
 * @func isInteger32
 * @aliases isInt32
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.32.0|v2.32.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.toInteger32|toInteger32}
 * @example
 *
 * RA.isInteger32(0); //=> true
 * RA.isInteger32((-2) ** 31); //=> true
 *
 * RA.isInteger32(Infinity); //=> false
 * RA.isInteger32(NaN); //=> false
 * RA.isInteger32(2 ** 31); //=> false
 */
var isInteger32 = (0, _ramda.curryN)(1, function (val) {
  return (0, _toInteger["default"])(val) === val;
});
var _default = exports["default"] = isInteger32;