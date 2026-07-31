"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Returns true if two lists have at least one element common to both lists.
 *
 * @func overlaps
 * @memberOf RA
 * @category Relation
 * @since {@link https://char0n.github.io/ramda-adjunct/2.30.0|v2.30.0}
 * @sig [a] -> [a] -> Boolean
 * @param {Array} list1 The first list
 * @param {Array} list2 The second list
 * @return {boolean} True if two lists have at least one element common to both lists
 * @example
 *
 * RA.overlaps(['-v', '--verbose'], ['node', 'script.js', '-v']); //=> true
 * RA.overlaps(['-v', '--verbose'], []); //=> false
 * RA.overlaps([1, 2, 3], [3, 4, 5]); //=> true
 * RA.overlaps([1, 2, 3], [4, 5]); //=> false
 */

var overlaps = (0, _ramda.curryN)(2, function (list1, list2) {
  if ((0, _ramda.isEmpty)(list1)) {
    return true;
  }
  return (0, _ramda.pipe)(_ramda.intersection, _ramda.isNotEmpty)(list1, list2);
});
var _default = exports["default"] = overlaps;