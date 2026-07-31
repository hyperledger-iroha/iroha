"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isPrimitive = _interopRequireDefault(require("./isPrimitive.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is not a primitive data type. There are 6 primitive data types: `string`, `number`, `bigint`, `boolean`, `undefined`, `symbol` and a special case of `null`.
 *
 * @func isNotPrimitive
 * @memberOf RA
 * @category Type
 * @sig * -> Boolean
 * @since {@link https://char0n.github.io/ramda-adjunct/2.32.0|v2.32.0}
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isPrimitive|isPrimitive}, {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#primitive_values|MDN Primitive values}, {@link https://developer.mozilla.org/en-US/docs/Glossary/Primitive|MDN Primitive}
 * @example
 *
 * RA.isNotPrimitive(new String("string")); //=> true
 * RA.isNotPrimitive(new Number(1)); //=> true
 * RA.isNotPrimitive("string"); //=> false
 * RA.isNotPrimitive(1); //=> false
 */

var isNotPrimitive = (0, _ramda.curryN)(1, (0, _ramda.complement)(_isPrimitive["default"]));
var _default = exports["default"] = isNotPrimitive;