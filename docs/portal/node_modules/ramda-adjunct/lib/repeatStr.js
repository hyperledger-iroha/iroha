"use strict";

exports.__esModule = true;
exports.repeatStrPonyfill = exports.repeatStrInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _StringRepeat = _interopRequireDefault(require("./internal/ponyfills/String.repeat.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var repeatStrPonyfill = exports.repeatStrPonyfill = (0, _ramda.curry)(_StringRepeat["default"]);
var repeatStrInvoker = exports.repeatStrInvoker = (0, _ramda.flip)((0, _ramda.invoker)(1, 'repeat'));

/**
 * Constructs and returns a new string which contains the specified
 * number of copies of the string on which it was called, concatenated together.
 *
 * @func repeatStr
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.11.0|v2.11.0}
 * @category List
 * @sig String -> Number -> String
 * @param {string} value String value to be repeated
 * @param {number} count An integer between 0 and +∞: [0, +∞), indicating the number of times to repeat the string in the newly-created string that is to be returned
 * @return {string} A new string containing the specified number of copies of the given string
 * @example
 *
 * RA.repeatStr('a', 3); //=> 'aaa'
 */
var repeatStr = (0, _isFunction["default"])(String.prototype.repeat) ? repeatStrInvoker : repeatStrPonyfill;
var _default = exports["default"] = repeatStr;