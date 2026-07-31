"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNumber = _interopRequireDefault(require("./isNumber.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is a positive `Number` primitive or object. Zero is not considered positive.
 *
 * @func isPositive
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.15.0|v1.15.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNegative|isNegative}
 * @example
 *
 * RA.isPositive(1); // => true
 * RA.isPositive(Number.MAX_VALUE); // => true
 * RA.isPositive(-Infinity); // => false
 * RA.isPositive(NaN); // => false
 * RA.isPositive('5'); // => false
 */
var isPositive = (0, _ramda.both)(_isNumber["default"], (0, _ramda.lt)(0));
var _default = exports["default"] = isPositive;