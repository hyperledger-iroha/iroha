"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFloat = _interopRequireDefault(require("./isFloat.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is complement of a `float`.
 *
 * @func isNotFloat
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.14.0|v1.14.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isFloat|isFloat}
 * @example
 *
 * RA.isNotFloat(0); //=> true
 * RA.isNotFloat(1); //=> true
 * RA.isNotFloat(-100000); //=> true
 *
 * RA.isNotFloat(0.1);       //=> false
 * RA.isNotFloat(Math.PI);   //=> false
 *
 * RA.isNotFloat(NaN);       //=> true
 * RA.isNotFloat(Infinity);  //=> true
 * RA.isNotFloat(-Infinity); //=> true
 * RA.isNotFloat('10');      //=> true
 * RA.isNotFloat(true);      //=> true
 * RA.isNotFloat(false);     //=> true
 * RA.isNotFloat([1]);       //=> true
 */
var isNotFloat = (0, _ramda.curryN)(1, (0, _ramda.complement)(_isFloat["default"]));
var _default = exports["default"] = isNotFloat;