"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Returns the value of a number rounded to the nearest integer.
 *
 * @func round
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.15.0|v2.15.0}
 * @category Math
 * @sig Number -> Number
 * @param {number} number The number to round
 * @return {number} The value of the given number rounded to the nearest integer
 * @example
 *
 * RA.round(0.9); //=> 1
 * RA.round(5.95); //=> 6
 * RA.round(5.5); //=> 6
 * RA.round(5.05); //=> 5
 * RA.round(-5.05); //=> -5
 * RA.round(-5.5); //=> -5
 * RA.round(-5.95); //=> -6
 */

var round = (0, _ramda.curryN)(1, (0, _ramda.bind)(Math.round, Math));
var _default = exports["default"] = round;