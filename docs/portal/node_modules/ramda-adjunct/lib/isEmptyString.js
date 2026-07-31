"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Checks if input value is an empty `String`.
 *
 * @func isEmptyString
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.4.0|v2.4.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotEmptyString|isNotEmptyString}
 * @example
 *
 * RA.isEmptyString(''); // => true
 * RA.isEmptyString('42'); // => false
 * RA.isEmptyString(new String('42')); // => false
 * RA.isEmptyString(new String('')); // => false
 * RA.isEmptyString([42]); // => false
 * RA.isEmptyString({}); // => false
 * RA.isEmptyString(null); // => false
 * RA.isEmptyString(undefined); // => false
 * RA.isEmptyString(42); // => false
 */
var isEmptyString = (0, _ramda.equals)('');
var _default = exports["default"] = isEmptyString;