"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/* eslint-disable max-len */
/**
 * Checks if input value is complement of `Function`.
 *
 * @func isNotFunction
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.5.0|v0.5.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isFunction|isFunction}, {@link RA.isAsyncFunction|isNotAsyncFunction}, {@link RA.isGeneratorFunction|isGeneratorFunction}
 * @example
 *
 * RA.isNotFunction(function test() { }); //=> false
 * RA.isNotFunction(function* test() { }); //=> false
 * RA.isNotFunction(async function test() { }); //=> false
 * RA.isNotFunction(() => {}); //=> false
 * RA.isNotFunction(null); //=> true
 * RA.isNotFunction('abc'); //=> true
 */
/* eslint-enable max-len */
var isNotFunction = (0, _ramda.complement)(_isFunction["default"]);
var _default = exports["default"] = isNotFunction;