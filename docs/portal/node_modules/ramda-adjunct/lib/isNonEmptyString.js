"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isString = _interopRequireDefault(require("./isString.js"));
var _isNotObj = _interopRequireDefault(require("./isNotObj.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if input value is not an empty `String`.
 *
 * @func isNonEmptyString
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.4.0|v2.4.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isEmptyString|isEmptyString}
 * @example
 *
 * RA.isNonEmptyString('42'); // => true
 * RA.isNonEmptyString(''); // => false
 * RA.isNonEmptyString(new String('42')); // => false
 * RA.isNonEmptyString(new String('')); // => false
 * RA.isNonEmptyString([42]); // => false
 * RA.isNonEmptyString({}); // => false
 * RA.isNonEmptyString(null); // => false
 * RA.isNonEmptyString(undefined); // => false
 * RA.isNonEmptyString(42); // => false
 */
var isNonEmptyString = (0, _ramda.allPass)([_isString["default"], _isNotObj["default"], _ramda.isNotEmpty]);
var _default = exports["default"] = isNonEmptyString;