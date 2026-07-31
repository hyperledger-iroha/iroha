"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isValidNumber = _interopRequireDefault(require("./isValidNumber.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is not a valid `Number`. A valid `Number` is a number that is not `NaN`,
 * `Infinity` or `-Infinity`.
 *
 * @func isNotValidNumber
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.2.0|v2.2.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isValidNumber|isValidNumber}
 * @example
 *
 * RA.isNotValidNumber(1); //=> false
 * RA.isNotValidNumber(''); //=> true
 * RA.isNotValidNumber(NaN); //=> true
 * RA.isNotValidNumber(Infinity); //=> true
 * RA.isNotValidNumber(-Infinity); //=> true
 */
var isNotValidNumber = (0, _ramda.complement)(_isValidNumber["default"]);
var _default = exports["default"] = isNotValidNumber;