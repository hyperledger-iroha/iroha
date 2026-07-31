"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _included = _interopRequireDefault(require("./included.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Removes specified characters from the beginning of a string.
 *
 * @func trimCharsStart
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.24.0|v2.24.0}
 * @category String
 * @sig String -> String
 * @param {string} chars The characters to trim
 * @param {string} value The string to trim
 * @return {string} Returns the trimmed string.
 * @example
 *
 * RA.trimCharsStart('_-', '-_-abc-_-'); //=> 'abc-_-'
 */

var trimCharsStart = (0, _ramda.curry)(function (chars, value) {
  return (0, _ramda.pipe)((0, _ramda.split)(''), (0, _ramda.dropWhile)((0, _included["default"])(chars)), (0, _ramda.join)(''))(value);
});
var _default = exports["default"] = trimCharsStart;