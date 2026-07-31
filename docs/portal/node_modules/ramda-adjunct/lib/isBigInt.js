"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Checks if value is a BigInt.
 *
 * @func isBigInt
 * @memberOf RA
 * @category Type
 * @since {@link https://char0n.github.io/ramda-adjunct/2.27.0|v2.27.0}
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @example
 *
 * RA.isBigInt(5); // => false
 * RA.isBigInt(Number.MAX_VALUE); // => false
 * RA.isBigInt(-Infinity); // => false
 * RA.isBigInt(10); // => false
 * RA.isBigInt(10n); // => true
 * RA.isBigInt(BitInt(9007199254740991)); // => true
 */
var isBigInt = (0, _ramda.curryN)(1, (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('BigInt')));
var _default = exports["default"] = isBigInt;