"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Returns the largest integer less than or equal to a given number.
 *
 * Note: floor(null) returns integer 0 and do not give a NaN error.
 *
 * @func floor
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.15.0|v2.15.0}
 * @category Math
 * @sig Number -> Number
 * @param {number} number The number to floor
 * @return {number} A number representing the largest integer less than or equal to the specified number
 * @example
 *
 * RA.floor(45.95); //=> 45
 * RA.floor(45.05); //=> 45
 * RA.floor(4); //=> 4
 * RA.floor(-45.05); //=> -46
 * RA.floor(-45.95); //=> -46
 * RA.floor(null); //=> 0
 */

var floor = (0, _ramda.curryN)(1, (0, _ramda.bind)(Math.floor, Math));
var _default = exports["default"] = floor;