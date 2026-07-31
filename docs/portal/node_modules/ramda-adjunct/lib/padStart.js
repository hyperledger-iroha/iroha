"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _padCharsStart = _interopRequireDefault(require("./padCharsStart.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Pads string on the left side if it's shorter than length.
 *
 * @func padStart
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.25.0|v2.25.0}
 * @category String
 * @sig Number -> String -> String
 * @param {number} targetLength The length of the resulting string once
 * the current string has been padded
 * @param {string} value String value to be padded
 * @return {string} A new string of the specified length with the empty string
 * applied to the beginning of the current string
 * @see {@link RA.padCharsEnd|padCharsEnd}, {@link RA.padCharsStart|padCharsStart}, {@link RA.padEnd|padEnd}
 * @example
 *
 * RA.padStart(3, 'a'); // => '  a'
 */
var padStart = (0, _padCharsStart["default"])(' ');
var _default = exports["default"] = padStart;