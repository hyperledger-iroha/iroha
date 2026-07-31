"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isString = _interopRequireDefault(require("./isString.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Escapes the RegExp special characters.
 *
 * @func escapeRegExp
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.21.0|v2.21.0}
 * @category String
 * @sig String -> String
 * @param {string} val the value to escape
 * @return {string}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions#Escaping|MDN Regular Expressions Escaping}
 * @example
 *
 * RA.escapeRegExp('[ramda-adjunct](https://github.com/char0n/ramda-adjunct)'); //=> '\[ramda\-adjunct\]\(https://github\.com/char0n/ramda\-adjunct\)'
 */
var escapeRegExp = (0, _ramda.when)(_isString["default"], (0, _ramda.replace)(/[.*+?^${}()|[\]\\-]/g, '\\$&'));
var _default = exports["default"] = escapeRegExp;