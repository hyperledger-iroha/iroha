"use strict";

exports.__esModule = true;
exports.signPonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _MathSign = _interopRequireDefault(require("./internal/ponyfills/Math.sign.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var signPonyfill = exports.signPonyfill = (0, _ramda.curryN)(1, _MathSign["default"]);

/**
 * Returns the sign of a number, indicating whether the number is positive, negative or zero.
 *
 * @func sign
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.15.0|v2.15.0}
 * @category Math
 * @sig Number | String -> Number
 * @param {number} number A number
 * @return {number} A number representing the sign of the given argument. If the argument is a positive number, negative number, positive zero or negative zero, the function will return 1, -1, 0 or -0 respectively. Otherwise, NaN is returned
 * @example
 *
 * RA.sign(3); //  1
 * RA.sign(-3); // -1
 * RA.sign('-3'); // -1
 * RA.sign(0); //  0
 * RA.sign(-0); // -0
 * RA.sign(NaN); // NaN
 * RA.sign('foo'); // NaN
 */

var sign = (0, _isFunction["default"])(Math.sign) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Math.sign, Math)) : signPonyfill;
var _default = exports["default"] = sign;