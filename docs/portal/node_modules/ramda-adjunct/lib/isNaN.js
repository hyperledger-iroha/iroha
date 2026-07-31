"use strict";

exports.__esModule = true;
exports.isNaNPonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _NumberIsNaN = _interopRequireDefault(require("./internal/ponyfills/Number.isNaN.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isNaNPonyfill = exports.isNaNPonyfill = (0, _ramda.curryN)(1, _NumberIsNaN["default"]);

/**
 * Checks whether the passed value is `NaN` and its type is `Number`.
 * It is a more robust version of the original, global isNaN().
 *
 *
 * @func isNaN
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.6.0|v0.6.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotNaN|isNotNaN}
 * @example
 *
 * RA.isNaN(NaN); // => true
 * RA.isNaN(Number.NaN); // => true
 * RA.isNaN(0 / 0); // => true
 *
 * // e.g. these would have been true with global isNaN().
 * RA.isNaN('NaN'); // => false
 * RA.isNaN(undefined); // => false
 * RA.isNaN({}); // => false
 * RA.isNaN('blabla'); // => false
 *
 * RA.isNaN(true); // => false
 * RA.isNaN(null); // => false
 * RA.isNaN(37); // => false
 * RA.isNaN('37'); // => false
 * RA.isNaN('37.37'); // => false
 * RA.isNaN(''); // => false
 * RA.isNaN(' '); // => false
 */
var _isNaN = (0, _isFunction["default"])(Number.isNaN) ? (0, _ramda.curryN)(1, Number.isNaN) : isNaNPonyfill;
var _default = exports["default"] = _isNaN;