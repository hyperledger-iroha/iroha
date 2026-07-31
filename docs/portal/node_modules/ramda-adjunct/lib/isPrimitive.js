"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNotObj = _interopRequireDefault(require("./isNotObj.js"));
var _isString = _interopRequireDefault(require("./isString.js"));
var _isNumber = _interopRequireDefault(require("./isNumber.js"));
var _isBigInt = _interopRequireDefault(require("./isBigInt.js"));
var _isBoolean = _interopRequireDefault(require("./isBoolean.js"));
var _isUndefined = _interopRequireDefault(require("./isUndefined.js"));
var _isNull = _interopRequireDefault(require("./isNull.js"));
var _isSymbol = _interopRequireDefault(require("./isSymbol.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is a primitive data type. There are 6 primitive data types: `string`, `number`, `bigint`, `boolean`, `undefined`, `symbol` and a special case of `null`.
 * See https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#Primitive_values
 * for definition of what sub-types comprise a primitive.
 *
 * @func isPrimitive
 * @memberOf RA
 * @category Type
 * @sig * -> Boolean
 * @since {@link https://char0n.github.io/ramda-adjunct/2.32.0|v2.32.0}
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotPrimitive|isNotPrimitive}, {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures#primitive_values|MDN Primitive values}, {@link https://developer.mozilla.org/en-US/docs/Glossary/Primitive|MDN Primitive}
 * @example
 *
 * RA.isPrimitive("string"); //=> true
 * RA.isPrimitive(1); //=> true
 * RA.isPrimitive(new String("string")); //=> false
 * RA.isPrimitive(new Number(1)); //=> false
 */

var isPrimitive = (0, _ramda.both)(_isNotObj["default"], (0, _ramda.anyPass)([_isString["default"], _isNumber["default"], _isBigInt["default"], _isBoolean["default"], _isUndefined["default"], _isNull["default"], _isSymbol["default"]]));
var _default = exports["default"] = isPrimitive;