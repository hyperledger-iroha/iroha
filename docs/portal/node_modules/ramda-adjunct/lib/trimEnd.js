"use strict";

exports.__esModule = true;
exports.trimEndPonyfill = exports.trimEndInvoker = exports["default"] = void 0;
var _ramda = require("ramda");
var _StringTrimEnd = _interopRequireDefault(require("./internal/ponyfills/String.trimEnd.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var trimEndPonyfill = exports.trimEndPonyfill = _StringTrimEnd["default"];
var trimEndInvoker = exports.trimEndInvoker = (0, _ramda.invoker)(0, 'trimEnd');

/**
 * Removes whitespace from the end of a string.
 *
 * @func trimEnd
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category String
 * @sig String -> String
 * @param {string} value String value to have the whitespace removed from the end
 * @return {string} A new string representing the calling string stripped of whitespace from its end (right end).
 * @see {@link RA.trimEnd|trimEnd}
 * @example
 *
 * RA.trimEnd('abc   '); //=> 'abc'
 */

var trimEnd = (0, _isFunction["default"])(String.prototype.trimEnd) ? trimEndInvoker : trimEndPonyfill;
var _default = exports["default"] = trimEnd;