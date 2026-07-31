"use strict";

exports.__esModule = true;
exports.truncPonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _MathTrunc = _interopRequireDefault(require("./internal/ponyfills/Math.trunc.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var truncPonyfill = exports.truncPonyfill = (0, _ramda.curryN)(1, _MathTrunc["default"]);

/**
 * Returns the integer part of a number by removing any fractional digits.
 *
 * @func trunc
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.15.0|v2.15.0}
 * @category Math
 * @sig Number | String -> Number
 * @param {number|string} number The number to trunc
 * @return {number} The integer part of the given number
 * @example
 *
 * RA.trunc(13.37); //=> 13
 * RA.trunc(42.84); //=> 42
 * RA.trunc(0.123); //=>  0
 * RA.trunc(-0.123); //=> -0
 * RA.trunc('-1.123'); //=> -1
 * RA.trunc(NaN); //=> NaN
 * RA.trunc('foo'); //=> NaN
 */

var trunc = (0, _isFunction["default"])(Math.trunc) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Math.trunc, Math)) : truncPonyfill;
var _default = exports["default"] = trunc;