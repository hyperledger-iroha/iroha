"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNumber = _interopRequireDefault(require("./isNumber.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is a non-negative `Number` primitive or object. This includes all positive
 * numbers and zero.
 *
 * @func isNonNegative
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.6.0|v2.6.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isPositive|isPositive}, {@link RA.isNonPositive|isNonPositive}
 * @example
 *
 * RA.isNonNegative(0); // => true
 * RA.isNonNegative(1); // => true
 * RA.isNonNegative(Infinity); // => true
 * RA.isNonNegative(Number.MAX_VALUE); // => true
 *
 * RA.isNonNegative(-Infinity); // => false
 * RA.isNonNegative(Number.MIN_VALUE); // => false
 * RA.isNonNegative(NaN); // => false
 */
var isNonNegative = (0, _ramda.curryN)(1, (0, _ramda.both)(_isNumber["default"], (0, _ramda.flip)(_ramda.gte)(0)));
var _default = exports["default"] = isNonNegative;