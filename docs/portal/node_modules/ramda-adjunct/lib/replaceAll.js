"use strict";

exports.__esModule = true;
exports.replaceAllPonyfill = exports.replaceAllInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _StringReplaceAll = _interopRequireDefault(require("./internal/ponyfills/String.replaceAll.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var replaceAllPonyfill = exports.replaceAllPonyfill = (0, _ramda.curryN)(3, _StringReplaceAll["default"]);
var replaceAllInvoker = exports.replaceAllInvoker = (0, _ramda.invoker)(2, 'replaceAll');

/**
 * Replaces all substring matches in a string with a replacement.
 *
 * @func replaceAll
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.17.0|v2.17.0}
 * @category String
 * @sig String -> String -> String -> String
 * @param {string} searchValue The substring or a global RegExp to match
 * @param {string} replaceValue The string to replace the matches with
 * @param {string} str The String to do the search and replacement in
 * @return {string} A new string containing all the `searchValue` replaced with the `replaceValue`
 * @throws {TypeError} When invalid arguments provided
 * @see {@link http://ramdajs.com/docs/#replace|R.replace}, {@link https://github.com/tc39/proposal-string-replaceall|TC39 proposal}
 * @example
 *
 * RA.replaceAll('ac', 'ef', 'ac ab ac ab'); //=> 'ef ab ef ab'
 * RA.replaceAll('', '_', 'xxx'); //=> '_x_x_x_'
 * RA.replaceAll(/x/g, 'v', 'xxx'); //=> 'vvv'
 * RA.replaceAll(/x/, 'v', 'xxx'); //=> TypeError
 */
var replaceAll = (0, _isFunction["default"])(String.prototype.replaceAll) ? replaceAllInvoker : replaceAllPonyfill;
var _default = exports["default"] = replaceAll;