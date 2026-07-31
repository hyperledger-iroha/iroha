"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isValidDate = _interopRequireDefault(require("./isValidDate.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is complement of valid `Date` object.
 *
 * @func isNotValidDate
 * @aliases isInvalidDate
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.8.0|v1.8.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isValidDate|isValidDate}, {@link RA.isDate|isDate}, {@link RA.isNotDate|isNotDate}
 * @example
 *
 * RA.isNotValidDate(new Date()); //=> false
 * RA.isNotValidDate(new Date('a')); //=> true
 */
var isNotValidDate = (0, _ramda.complement)(_isValidDate["default"]);
var _default = exports["default"] = isNotValidDate;