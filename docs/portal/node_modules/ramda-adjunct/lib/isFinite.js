"use strict";

exports.__esModule = true;
exports.isFinitePonyfill = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _NumberIsFinite = _interopRequireDefault(require("./internal/ponyfills/Number.isFinite.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isFinitePonyfill = exports.isFinitePonyfill = (0, _ramda.curryN)(1, _NumberIsFinite["default"]);

/**
 * Checks whether the passed value is a finite `Number`.
 *
 * @func isFinite
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.7.0|v0.7.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotFinite|isNotFinite}
 * @example
 *
 * RA.isFinite(Infinity); //=> false
 * RA.isFinite(NaN); //=> false
 * RA.isFinite(-Infinity); //=> false
 *
 * RA.isFinite(0); // true
 * RA.isFinite(2e64); // true
 *
 * RA.isFinite('0');  // => false
 *                    // would've been true with global isFinite('0')
 * RA.isFinite(null); // => false
 *                    // would've been true with global isFinite(null)
 */
var _isFinite = (0, _isFunction["default"])(Number.isFinite) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Number.isFinite, Number)) : isFinitePonyfill;
var _default = exports["default"] = _isFinite;