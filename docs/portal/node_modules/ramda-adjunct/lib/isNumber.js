"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Checks if value is a `Number` primitive or object.
 *
 * @func isNumber
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.6.0|v0.6.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotNumber|isNotNumber}
 * @example
 *
 * RA.isNumber(5); // => true
 * RA.isNumber(Number.MAX_VALUE); // => true
 * RA.isNumber(-Infinity); // => true
 * RA.isNumber(NaN); // => true
 * RA.isNumber('5'); // => false
 */
var isNumber = (0, _ramda.curryN)(1, (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Number')));
var _default = exports["default"] = isNumber;