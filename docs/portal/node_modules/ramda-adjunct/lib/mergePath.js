"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Create a new object with the own properties of the object under the `path`
 * merged with the own properties of the provided `source`.
 * If a key exists in both objects, the value from the `source` object will be used.
 *
 * @func mergePath
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.18.0|v1.18.0}
 * @category Object
 * @sig [k] -> {a} -> {k: {a}} -> {k: {a}}
 * @see {@link RA.mergeProp|mergeProp}
 * @param {!Array} path The property path of the destination object
 * @param {!Object} source The source object
 * @param {!Object} obj The object that has destination object under corresponding property path
 * @return {!Object} The new version of object
 * @example
 *
 * RA.mergePath(
 *  ['outer', 'inner'],
 *  { foo: 3, bar: 4 },
 *  { outer: { inner: { foo: 2 } } }
 * ); //=> { outer: { inner: { foo: 3, bar: 4 } }
 */
var mergePath = (0, _ramda.curry)(function (path, source, obj) {
  return (0, _ramda.over)((0, _ramda.lensPath)(path), (0, _ramda.mergeLeft)(source), obj);
});
var _default = exports["default"] = mergePath;