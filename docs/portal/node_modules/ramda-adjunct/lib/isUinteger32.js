"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _toUinteger = _interopRequireDefault(require("./toUinteger32.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is an unsigned 32 bit integer.
 *
 * @func isUinteger32
 * @aliases isUint32
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/3.2.0|v3.2.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.toUinteger32|toUinteger32}
 * @example
 *
 * RA.isUinteger32(0); //=> true
 * RA.isUinteger32(2 ** 32 - 1); //=> true
 *
 * RA.isUinteger32(Infinity); //=> false
 * RA.isUinteger32(NaN); //=> false
 * RA.isUinteger32(-1); //=> false
 * RA.isUinteger32(2 ** 32); //=> false
 */
var isUinteger32 = (0, _ramda.curryN)(1, function (val) {
  return (0, _toUinteger["default"])(val) === val;
});
var _default = exports["default"] = isUinteger32;