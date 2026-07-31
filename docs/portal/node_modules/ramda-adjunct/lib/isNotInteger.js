"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("./isInteger.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is complement of an `integer`.
 *
 *
 * @func isNotInteger
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.7.0|v0.7.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isInteger|isInteger}
 * @example
 *
 * RA.isNotInteger(0); //=> false
 * RA.isNotInteger(1); //=> false
 * RA.isNotInteger(-100000); //=> false
 *
 * RA.isNotInteger(0.1);       //=> true
 * RA.isNotInteger(Math.PI);   //=> true
 *
 * RA.isNotInteger(NaN);       //=> true
 * RA.isNotInteger(Infinity);  //=> true
 * RA.isNotInteger(-Infinity); //=> true
 * RA.isNotInteger('10');      //=> true
 * RA.isNotInteger(true);      //=> true
 * RA.isNotInteger(false);     //=> true
 * RA.isNotInteger([1]);       //=> true
 */
var isNotInteger = (0, _ramda.complement)(_isInteger["default"]);
var _default = exports["default"] = isNotInteger;