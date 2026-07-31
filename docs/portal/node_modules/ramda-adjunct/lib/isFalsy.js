"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isTruthy = _interopRequireDefault(require("./isTruthy.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * A falsy value is a value that translates to false when evaluated in a Boolean context.
 * Falsy values are `false`, `0`, `""`, `null`, `undefined`, and `NaN`.
 *
 * @func isFalsy
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.2.0|v2.2..0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link https://developer.mozilla.org/en-US/docs/Glossary/Falsy|falsy}, {@link RA.isTruthy|isTruthy}
 * @example
 *
 * RA.isFalsy(false); // => true
 * RA.isFalsy(0); // => true
 * RA.isFalsy(''); // => true
 * RA.isFalsy(null); // => true
 * RA.isFalsy(undefined); // => true
 * RA.isFalsy(NaN); // => true
 */
var isFalsy = (0, _ramda.complement)(_isTruthy["default"]);
var _default = exports["default"] = isFalsy;