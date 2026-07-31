"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isRegExp = _interopRequireDefault(require("./isRegExp.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is complement of `RegExp` object.
 *
 * @func isNotRegExp
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.5.0|v2.5.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isRegExp|isRegExp}
 * @example
 *
 * RA.isNotRegExp(1); //=> true
 * RA.isNotRegExp(/(?:)/); //=> false
 * RA.isNotRegExp(new RegExp()); //=> false
 */
var isNotRegExp = (0, _ramda.complement)(_isRegExp["default"]);
var _default = exports["default"] = isNotRegExp;