"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _lengthEq = _interopRequireDefault(require("./lengthEq.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns true if all items in the list are unique. `R.equals` is used to determine equality.
 *
 * @func allUnique
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category List
 * @sig [a] -> Boolean
 * @param {Array} list The list of values
 * @return {boolean}
 * @see {@link RA.notAllUnique|notAllUnique},  {@link https://ramdajs.com/docs/#equals|equals}
 * @example
 *
 * RA.allUnique([ 1, 2, 3, 4 ]); //=> true
 * RA.allUnique([ 1, 1, 2, 3 ]); //=> false
 * RA.allUnique([]); //=> true
 *
 */
var allUnique = (0, _ramda.converge)(_lengthEq["default"], [_ramda.length, _ramda.uniq]);
var _default = exports["default"] = allUnique;