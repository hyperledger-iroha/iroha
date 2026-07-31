"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if input value is `Array`.
 *
 * @func isArray
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.3.0|v0.3.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotArray|isNotArray}
 * @example
 *
 * RA.isArray([]); //=> true
 * RA.isArray(null); //=> false
 * RA.isArray({}); //=> false
 */
var isArray = (0, _ramda.curryN)(1, (0, _isFunction["default"])(Array.isArray) ? Array.isArray : (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Array')));
var _default = exports["default"] = isArray;