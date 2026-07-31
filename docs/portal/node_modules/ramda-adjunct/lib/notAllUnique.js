"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _allUnique = _interopRequireDefault(require("./allUnique.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns true if at least one item of the list is repeated. `R.equals` is used to determine equality.
 *
 * @func notAllUnique
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category List
 * @sig [a] -> Boolean
 * @param {Array} list The list of values
 * @return {boolean}
 * @see {@link RA.allUnique|allUnique}, {@link https://ramdajs.com/docs/#equals|equals}
 * @example
 *
 * RA.notAllUnique([ 1, 1, 2, 3 ]); //=> true
 * RA.notAllUnique([ 1, 2, 3, 4 ]); //=> false
 * RA.notAllUnique([]); //=> false
 *
 */
var notAllUnique = (0, _ramda.complement)(_allUnique["default"]);
var _default = exports["default"] = notAllUnique;