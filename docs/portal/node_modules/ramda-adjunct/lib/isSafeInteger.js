"use strict";

exports.__esModule = true;
exports.isSafeIntegerPonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _NumberIsSafeInteger = _interopRequireDefault(require("./internal/ponyfills/Number.isSafeInteger.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isSafeIntegerPonyfill = exports.isSafeIntegerPonyfill = (0, _ramda.curryN)(1, _NumberIsSafeInteger["default"]);

/**
 * Checks whether the passed value is a safe `integer`.
 *
 * @func isSafeInteger
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @example
 *
 * RA.isSafeInteger(3); //=> true
 * RA.isSafeInteger(Math.pow(2, 53)) //=> false
 * RA.isSafeInteger(Math.pow(2, 53) - 1); //=> true
 * RA.isSafeInteger(NaN); //=> false
 * RA.isSafeInteger(Infinity); //=> false
 * RA.isSafeInteger('3') //=> false
 * RA.isSafeInteger(3.1); //=> false
 * RA.isSafeInteger(3.0); //=> true
 * RA.isSafeInteger('string'); //=> false
 * RA.isSafeInteger(null); //=> false
 * RA.isSafeInteger(undefined); //=> false
 * RA.isSafeInteger({}); //=> false
 * RA.isSafeInteger(() => { }); //=> false
 * RA.isSafeInteger(true); //=> false
 */

var isSafeInteger = (0, _isFunction["default"])(Number.isSafeInteger) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Number.isSafeInteger, Number)) : isSafeIntegerPonyfill;
var _default = exports["default"] = isSafeInteger;