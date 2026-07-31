"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _lengthLte = _interopRequireDefault(require("./lengthLte.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
// Original idea for this function was conceived by https://github.com/jackmellis
// in https://github.com/char0n/ramda-adjunct/pull/513.

/**
 * Returns true if all items in the list are equivalent using `R.equals` for equality comparisons.
 *
 * @func allEqual
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.9.0|v2.9.0}
 * @category List
 * @sig [a] -> Boolean
 * @param {Array} list The list of values
 * @return {boolean}
 * @see {@link https://ramdajs.com/docs/#equals|equals}
 * @example
 *
 * RA.allEqual([ 1, 2, 3, 4 ]); //=> false
 * RA.allEqual([ 1, 1, 1, 1 ]); //=> true
 * RA.allEqual([]); //=> true
 *
 */
var allEqual = (0, _ramda.curryN)(1, (0, _ramda.pipe)(_ramda.uniq, (0, _lengthLte["default"])(1)));
var _default = exports["default"] = allEqual;