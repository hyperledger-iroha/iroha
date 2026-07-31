"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isArray = _interopRequireDefault(require("./isArray.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if input value is a pair.
 *
 * @func isPair
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.19.0|v1.19.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link http://ramdajs.com/docs/#pair|R.pair}, {@link RA.isNotPair|isNotPair}
 * @example
 *
 * RA.isPair([]); // => false
 * RA.isPair([0]); // => false
 * RA.isPair([0, 1]); // => true
 * RA.isPair([0, 1, 2]); // => false
 * RA.isPair({ 0: 0, 1: 1 }); // => false
 * RA.isPair({ foo: 0, bar: 0 }); // => false
 */
var isPair = (0, _ramda.curryN)(1, (0, _ramda.both)(_isArray["default"], (0, _ramda.pipe)(_ramda.length, (0, _ramda.equals)(2))));
var _default = exports["default"] = isPair;