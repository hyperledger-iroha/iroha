"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isGeneratorFunction = _interopRequireDefault(require("./isGeneratorFunction.js"));
var _isAsyncFunction = _interopRequireDefault(require("./isAsyncFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if input value is `Function`.
 *
 * @func isFunction
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.5.0|v0.5.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotFunction|isNotFunction}, {@link RA.isAsyncFunction|isNotAsyncFunction}, {@link RA.isGeneratorFunction|isGeneratorFunction}
 * @example
 *
 * RA.isFunction(function test() { }); //=> true
 * RA.isFunction(function* test() { }); //=> true
 * RA.isFunction(async function test() { }); //=> true
 * RA.isFunction(() => {}); //=> true
 * RA.isFunction(null); //=> false
 * RA.isFunction('abc'); //=> false
 */
var isFunction = (0, _ramda.anyPass)([(0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Function')), _isGeneratorFunction["default"], _isAsyncFunction["default"]]);
var _default = exports["default"] = isFunction;