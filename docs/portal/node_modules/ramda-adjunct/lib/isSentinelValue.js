"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("./isInteger32.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is {@link https://github.com/getify/You-Dont-Know-JS/blob/9959fc904d584bbf0b02cf41c192f74ff4238581/types-grammar/ch4.md#the-curious-case-of-the-|a sentinel value}.
 *
 * @func isSentinelValue
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.33.0|v2.33.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @example
 *
 * RA.isSentinelValue(-1); //=> true
 *
 * RA.isSentinelValue('-1'); //=> false
 * RA.isSentinelValue(1); //=> false
 * RA.isSentinelValue([-1]); //=> false
 */
// eslint-disable-next-line no-bitwise
var isSentinelValue = (0, _ramda.curryN)(1, function (val) {
  return (0, _isInteger["default"])(val) && ~val === 0;
});
var _default = exports["default"] = isSentinelValue;