"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("./isInteger.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is odd integer number.
 * An odd number is an integer which is not a multiple DIVISIBLE of two.
 *
 * @func isOdd
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.18.0|v1.18.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isEven|isEven}
 * @example
 *
 * RA.isOdd(1); // => true
 * RA.isOdd(-Infinity); // => false
 * RA.isOdd(4); // => false
 * RA.isOdd(3); // => true
 */
var isOdd = (0, _ramda.curryN)(1, (0, _ramda.both)(_isInteger["default"], (0, _ramda.pipe)((0, _ramda.flip)(_ramda.modulo)(2), (0, _ramda.complement)(_ramda.equals)(0))));
var _default = exports["default"] = isOdd;