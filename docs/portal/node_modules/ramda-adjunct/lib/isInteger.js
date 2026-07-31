"use strict";

exports.__esModule = true;
exports.isIntegerPonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _NumberIsInteger = _interopRequireDefault(require("./internal/ponyfills/Number.isInteger.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isIntegerPonyfill = exports.isIntegerPonyfill = (0, _ramda.curryN)(1, _NumberIsInteger["default"]);

/**
 * Checks whether the passed value is an `integer`.
 *
 * @func isInteger
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.7.0|v0.7.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotInteger|isNotInteger}
 * @example
 *
 * RA.isInteger(0); //=> true
 * RA.isInteger(1); //=> true
 * RA.isInteger(-100000); //=> true
 *
 * RA.isInteger(0.1);       //=> false
 * RA.isInteger(Math.PI);   //=> false
 *
 * RA.isInteger(NaN);       //=> false
 * RA.isInteger(Infinity);  //=> false
 * RA.isInteger(-Infinity); //=> false
 * RA.isInteger('10');      //=> false
 * RA.isInteger(true);      //=> false
 * RA.isInteger(false);     //=> false
 * RA.isInteger([1]);       //=> false
 */
var isInteger = (0, _isFunction["default"])(Number.isInteger) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Number.isInteger, Number)) : isIntegerPonyfill;
var _default = exports["default"] = isInteger;