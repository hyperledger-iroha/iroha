"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("./isInteger.js"));
var _isNegative = _interopRequireDefault(require("./isNegative.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is a natural number.
 * Natural numbers correspond to all non-negative integers and 0.
 *
 * @func isNaturalNumber
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.29.0|v2.29.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @example
 *
 * RA.isNaturalNumber(5); // => true
 * RA.isNaturalNumber(Number.MAX_VALUE); // => true
 * RA.isNaturalNumber(0); // => true
 * RA.isNaturalNumber(-1); // => false
 * RA.isNaturalNumber(0.9); // => false
 */

var isNaturalNumber = (0, _ramda.curryN)(1, (0, _ramda.both)(_isInteger["default"], (0, _ramda.complement)(_isNegative["default"])));
var _default = exports["default"] = isNaturalNumber;