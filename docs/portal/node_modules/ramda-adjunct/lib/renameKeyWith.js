"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _renameKeysWith = _interopRequireDefault(require("./renameKeysWith.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Creates a new object with the own properties of the provided object, but the
 * key `key` renamed according to logic of renaming function.
 *
 * Keep in mind that in case the new key name already existed on the object,
 * the behaviour is undefined and the result may vary between various JS engines!
 *
 * @func renameKeyWith
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.29.0|v2.29.0}
 * @category Object
 * @sig (k -> k) -> k -> {k: v} -> {k: v}
 * @param {Function} fn Function that renames the keys
 * @param {!string} key Key to rename
 * @param {!Object} obj Provided object
 * @return {!Object} New object with renamed key
 * @see {@link RA.renameKeysWith|renameKeysWith}
 * @example
 *
 * RA.renameKeyWith(R.concat('a'), 'A', { A: 1 }) //=> { aA: 1 }
 */
var renameKeyWith = (0, _ramda.curry)(function (fn, key, obj) {
  return (0, _renameKeysWith["default"])((0, _ramda.when)((0, _ramda.equals)(key), fn), obj);
});
var _default = exports["default"] = renameKeyWith;