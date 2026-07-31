"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isCoercible = _interopRequireDefault(require("./internal/isCoercible.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Converts value to a number.
 *
 * @func toNumber
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.36.0|v2.36.0}
 * @category Type
 * @param {*} val The value to convert
 * @return {Number}
 * @example
 *
 * RA.toNumber(3.2); //=> 3.2
 * RA.toNumber(Number.MIN_VALUE); //=> 5e-324
 * RA.toNumber(Infinity); //=> Infinity
 * RA.toNumber('3.2'); //=> 3.2
 * RA.toNumber(Symbol('3.2')); //=> NaN
 */
var toNumber = (0, _ramda.ifElse)(_isCoercible["default"], Number, (0, _ramda.always)(NaN));
var _default = exports["default"] = toNumber;