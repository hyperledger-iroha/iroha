"use strict";

exports.__esModule = true;
exports.trimStartPonyfill = exports.trimStartInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _StringTrimStart = _interopRequireDefault(require("./internal/ponyfills/String.trimStart.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var trimStartPonyfill = exports.trimStartPonyfill = _StringTrimStart["default"];
var trimStartInvoker = exports.trimStartInvoker = (0, _ramda.invoker)(0, 'trimStart');

/**
 * Removes whitespace from the beginning of a string.
 *
 * @func trimStart
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category String
 * @sig String -> String
 * @param {string} value String value to have the whitespace removed from the beginning
 * @return {string} A new string representing the calling string stripped of whitespace from its beginning (left end).
 * @example
 *
 * RA.trimStart('  abc'); //=> 'abc'
 */

var trimStart = (0, _isFunction["default"])(String.prototype.trimStart) ? trimStartInvoker : trimStartPonyfill;
var _default = exports["default"] = trimStart;