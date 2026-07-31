"use strict";

exports.__esModule = true;
exports.padEndPonyfill = exports.padEndInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _StringPadEnd = _interopRequireDefault(require("./internal/ponyfills/String.padEnd.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var padEndPonyfill = exports.padEndPonyfill = (0, _ramda.curry)(_StringPadEnd["default"]);
var padEndInvoker = exports.padEndInvoker = (0, _ramda.flip)((0, _ramda.invoker)(2, 'padEnd'));

/**
 * The function pads the current string with a given string
 * (repeated, if needed) so that the resulting string reaches a given length.
 * The padding is applied from the end of the current string.
 *
 * @func padCharsEnd
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category String
 * @sig String -> Number -> String -> String
 * @param {string} padString The string to pad the current string with
 * @param {number} targetLength The length of the resulting string once
 * the current string has been padded
 * @param {string} value String value to be padded
 * @return {string} A new string of the specified length with the pad string
 * applied at the end of the current string
 * @see {@link RA.padEnd|padEnd}, {@link RA.padCharsStart|padCharsStart}, {@link RA.padStart|padStart}
 * @example
 *
 * RA.padCharsEnd('-', 3, 'a'); // => 'a--'
 * RA.padCharsEnd('foo', 10, 'abc'); // => 'abcfoofoof'
 * RA.padCharsEnd('123456', 6, 'abc'); // => 'abc123'
 */
var padCharsEnd = (0, _isFunction["default"])(String.prototype.padEnd) ? padEndInvoker : padEndPonyfill;
var _default = exports["default"] = padCharsEnd;