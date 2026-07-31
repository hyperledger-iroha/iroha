"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNumber = _interopRequireDefault(require("./isNumber.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is a non-positive `Number` primitive or object. This includes all negative
 * numbers and zero.
 *
 * @func isNonPositive
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.6.0|v2.6.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNegative|isNegative}, {@link RA.isNonNegative|isNonNegative}
 * @example
 *
 * RA.isNonPositive(0); // => true
 * RA.isNonPositive(-1); // => true
 * RA.isNonPositive(-Infinity); // => true
 * RA.isNonPositive(Number.MIN_VALUE); // => true
 *
 * RA.isNonPositive(Infinity); // => false
 * RA.isNonPositive(Number.MAX_VALUE); // => false
 * RA.isNonPositive(NaN); // => false
 */
var isNonPositive = (0, _ramda.curryN)(1, (0, _ramda.both)(_isNumber["default"], (0, _ramda.flip)(_ramda.lte)(0)));
var _default = exports["default"] = isNonPositive;