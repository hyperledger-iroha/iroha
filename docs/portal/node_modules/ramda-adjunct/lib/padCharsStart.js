"use strict";

exports.__esModule = true;
exports.padStartPonyfill = exports.padStartInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _StringPadStart = _interopRequireDefault(require("./internal/ponyfills/String.padStart.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var padStartInvoker = exports.padStartInvoker = (0, _ramda.flip)((0, _ramda.invoker)(2, 'padStart'));
var padStartPonyfill = exports.padStartPonyfill = (0, _ramda.curry)(_StringPadStart["default"]);

/**
 * The function pads the current string with a given string
 * (repeated, if needed) so that the resulting string reaches a given length.
 * The padding is applied from the start of the current string.
 *
 * @func padCharsStart
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category String
 * @sig String -> Number -> String -> String
 * @param {string} padString The string to pad the current string with
 * @param {number} targetLength The length of the resulting string once the current string has been padded
 * @param {string} value String value to be padded
 * @return {string} A new string of the specified length with the pad string on the start of current string
 * @see {@link RA.padStart|padStart}, {@link RA.padEnd|padEnd}, {@link RA.padCharsEnd|padCharsEnd}
 * @example
 *
 * RA.padCharsStart('-', 3, 'a'); // => '--a'
 * RA.padCharsStart('foo', 10, 'abc'); // => 'foofoofabc'
 * RA.padCharsStart('123456', 6, 'abc'); // => '123abc'
 */
var padCharsStart = (0, _isFunction["default"])(String.prototype.padStart) ? padStartInvoker : padStartPonyfill;
var _default = exports["default"] = padCharsStart;