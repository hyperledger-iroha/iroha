"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _curryRightN = _interopRequireDefault(require("./curryRightN.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns a curried equivalent of the provided function.
 * This function is like curry, except that the provided arguments order is reversed.
 *
 * @func curryRight
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.12.0|v1.12.0}
 * @category Function
 * @sig (* -> a) -> (* -> a)
 * @param {Function} fn The function to curry
 * @return {Function}  A new, curried function
 * @see {@link http://ramdajs.com/docs/#curry|R.curry}, {@link RA.curryRightN|curryRightN}
 * @example
 *
 * const concatStrings = (a, b, c) => a + b + c;
 * const concatStringsCurried = RA.curryRight(concatStrings);
 *
 * concatStringCurried('a')('b')('c'); // => 'cba'
 */
var curryRight = (0, _ramda.converge)(_curryRightN["default"], [_ramda.length, _ramda.identity]);
var _default = exports["default"] = curryRight;