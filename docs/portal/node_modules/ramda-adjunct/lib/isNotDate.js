"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isDate = _interopRequireDefault(require("./isDate.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is complement of `Date` object.
 *
 * @func isNotDate
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.6.0|v0.6.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isDate|isDate}
 * @example
 *
 * RA.isNotDate(new Date()); //=> false
 * RA.isNotDate('1997-07-16T19:20+01:00'); //=> true
 */
var isNotDate = (0, _ramda.complement)(_isDate["default"]);
var _default = exports["default"] = isNotDate;