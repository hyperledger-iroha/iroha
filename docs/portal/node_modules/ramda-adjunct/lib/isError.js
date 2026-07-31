"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Checks if `value` is an `Error`, `EvalError`, `RangeError`, `ReferenceError`, `SyntaxError`, `TypeError` or `URIError` object.
 *
 * @func isError
 * @category Type
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.30.0|v2.30.0}
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean} Returns `true` if value is an error object, `false` otherwise
 * @example
 *
 * RA.isError(new Error()); //=> true
 * RA.isError(Error); //=> false
 * RA.isError(1); // => false
 */
var isError = (0, _ramda.curryN)(1, (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Error')));
var _default = exports["default"] = isError;