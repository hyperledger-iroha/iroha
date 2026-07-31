"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Flattens a property path so that its fields are spread out into the provided object.
 * It's like {@link RA.spreadPath|spreadPath}, but preserves object under the property path.
 *
 * @func flattenPath
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.19.0|v1.19.0}
 * @category Object
 * @typedef Idx = String | Int
 * @sig [Idx] -> {k: v} -> {k: v}
 * @param {!Array.<string|number>} path The property path to flatten
 * @param {!Object} obj The provided object
 * @return {!Object} The flattened object
 * @see {@link RA.flattenProp|flattenProp}, {@link RA.spreadPath|spreadPath}
 * @example
 *
 * RA.flattenPath(
 *   ['b1', 'b2'],
 *   { a: 1, b1: { b2: { c: 3, d: 4 } } }
 * ); // => { a: 1, c: 3, d: 4, b1: { b2: { c: 3, d: 4 } } };
 */
var flattenPath = (0, _ramda.curry)(function (path, obj) {
  return (0, _ramda.mergeRight)(obj, (0, _ramda.pathOr)({}, path, obj));
});
var _default = exports["default"] = flattenPath;