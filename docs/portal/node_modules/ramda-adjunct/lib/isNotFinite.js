"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFinite2 = _interopRequireDefault(require("./isFinite.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is complement of finite `Number`.
 *
 *
 * @func isNotFinite
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.7.0|v0.7.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isFinite|isFinite}
 * @example
 *
 * RA.isNotFinite(Infinity); //=> true
 * RA.isNotFinite(NaN); //=> true
 * RA.isNotFinite(-Infinity); //=> true
 *
 * RA.isNotFinite(0); // false
 * RA.isNotFinite(2e64); // false
 *
 * RA.isNotFinite('0');  // => true
 * RA.isNotFinite(null); // => true
 */
var isNotFinite = (0, _ramda.complement)(_isFinite2["default"]);
var _default = exports["default"] = isNotFinite;