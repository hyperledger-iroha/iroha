"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var inRangeImp = (0, _ramda.ifElse)(_ramda.gte, function () {
  throw new Error('low must not be greater than high in inRange(low, high, value)');
}, (0, _ramda.useWith)(_ramda.both, [_ramda.lte, _ramda.gt]));

/**
 * Checks if `value` is between `low` and up to but not including `high`.
 *
 * @func inRange
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.7.0|v2.7.0}
 * @category Relation
 * @sig Number -> Number -> Number -> Boolean
 * @param {number} low Start of the range
 * @param {number} high The end of the range
 * @param {number} value The value to test
 * @return {boolean}
 * @throws {Error} When `low` is greater than or equal to `high`
 * @example
 *
 * RA.inRange(0, 5, 3); //=> true
 * RA.inRange(0, 5, 0); //=> true
 * RA.inRange(0, 5, 4); //=> true
 * RA.inRange(0, 5, 5); //=> false
 * RA.inRange(0, 5, -1); //=> false
 */
var _default = exports["default"] = (0, _ramda.curry)(function (low, high, value) {
  return inRangeImp(low, high)(value);
});