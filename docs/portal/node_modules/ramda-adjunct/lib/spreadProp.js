"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _spreadPath = _interopRequireDefault(require("./spreadPath.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Spreads object under property onto provided object.
 * It's like {@link RA.flattenProp|flattenProp}, but removes object under the property.
 *
 * @func spreadProp
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.19.0|v1.19.0}
 * @category Object
 * @typedef Idx = String | Int
 * @sig Idx -> {k: v} -> {k: v}
 * @param {!string|number} prop The property to spread
 * @param {!Object} obj The provided object
 * @return {!Object} The result of the spread
 * @see {@link RA.spreadPath|spreadPath}, {@link RA.flattenProp|flattenProp}
 * @example
 *
 * RA.spreadProp('b', { a: 1, b: { c: 3, d: 4 } }); // => { a: 1, c: 3, d: 4 };
 */
var spreadProp = (0, _ramda.curry)(function (prop, obj) {
  return (0, _spreadPath["default"])((0, _ramda.of)(Array, prop), obj);
});
var _default = exports["default"] = spreadProp;