"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("./isInteger.js"));
var _isFinite = _interopRequireDefault(require("./isFinite.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is a `float`.
 *
 * @func isFloat
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.14.0|v1.14.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotFloat|isNotFloat}
 * @example
 *
 * RA.isFloat(0); //=> false
 * RA.isFloat(1); //=> false
 * RA.isFloat(-100000); //=> false
 *
 * RA.isFloat(0.1);       //=> true
 * RA.isFloat(Math.PI);   //=> true
 *
 * RA.isFloat(NaN);       //=> false
 * RA.isFloat(Infinity);  //=> false
 * RA.isFloat(-Infinity); //=> false
 * RA.isFloat('10');      //=> false
 * RA.isFloat(true);      //=> false
 * RA.isFloat(false);     //=> false
 * RA.isFloat([1]);       //=> false
 */
var isFloat = (0, _ramda.both)(_isFinite["default"], (0, _ramda.complement)(_isInteger["default"]));
var _default = exports["default"] = isFloat;