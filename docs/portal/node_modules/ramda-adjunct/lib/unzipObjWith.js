"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _lengthEq = _interopRequireDefault(require("./lengthEq.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Creates a new list out of the supplied object by applying the function to each key/value pairing.
 *
 * @func unzipObjWith
 * @memberOf RA
 * @category Object
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @sig  (v, k) => [k, v] -> { k: v } -> [[k], [v]]
 * @param {Function} fn The function to transform each value-key pair
 * @param {Object} obj Object to unzip
 * @return {Array} A pair of tw lists: a list of keys and a list of values
 * @see {@link https://ramdajs.com/docs/#zipObj|zipObj}, {@link RA.zipObjWith|zipObjWith}
 * @example
 *
 * RA.unzipObjWith((v, k) => [`new${k.toUpperCase()}`, 2 * v], { a: 1, b: 2, c: 3 });
 * //=> [['newA', 'newB', 'newC'], [2, 4, 6]]
 */
var unzipObjWith = (0, _ramda.curryN)(2, function (fn, obj) {
  return (0, _ramda.pipe)(_ramda.toPairs, (0, _ramda.map)((0, _ramda.pipe)(_ramda.flip, _ramda.apply)(fn)), _ramda.transpose, (0, _ramda.when)((0, _lengthEq["default"])(0), function () {
    return [[], []];
  }))(obj);
});
var _default = exports["default"] = unzipObjWith;