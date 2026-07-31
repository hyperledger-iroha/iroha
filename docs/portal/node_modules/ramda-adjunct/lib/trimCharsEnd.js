"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _included = _interopRequireDefault(require("./included.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Removes specified characters from the end of a string.
 *
 * @func trimCharsEnd
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.25.0|v2.25.0}
 * @category String
 * @sig String -> String
 * @param {string} chars The characters to trim
 * @param {string} value The string to trim
 * @return {string} Returns the trimmed string.
 * @example
 *
 * RA.trimCharsEnd('_-', '-_-abc-_-'); //=> '-_-abc'
 */

var trimCharsEnd = (0, _ramda.curry)(function (chars, value) {
  return (0, _ramda.pipe)((0, _ramda.split)(''), (0, _ramda.dropLastWhile)((0, _included["default"])(chars)), (0, _ramda.join)(''))(value);
});
var _default = exports["default"] = trimCharsEnd;