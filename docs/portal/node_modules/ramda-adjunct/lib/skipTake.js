"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * When given a number n and an array, returns an array containing every nth element.
 *
 * @func skipTake
 * @memberOf RA
 * @category List
 * @since {@link https://char0n.github.io/ramda-adjunct/2.26.0|v2.26.0}
 * @sig Number -> [a] -> [a]
 * @param {number} the nth element to extract
 * @param {Array} value the input array
 * @return {Array} An array containing every nth element
 * @example
 *
 * RA.skipTake(2, [1,2,3,4]) //=> [1, 3]
 * RA.skipTake(3, R.range(0, 20)); //=> [0, 3, 6, 9, 12, 15, 18]
 */

var skipTake = (0, _ramda.curry)(function (n, list) {
  return (0, _ramda.addIndex)(_ramda.filter)((0, _ramda.pipe)((0, _ramda.nthArg)(1), (0, _ramda.modulo)(_ramda.__, n), (0, _ramda.identical)(0)))(list);
});
var _default = exports["default"] = skipTake;