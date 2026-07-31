"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
/**
 * Checks if input value is a Symbol.
 *
 * @func isSymbol
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Symbol|MDN Symbol}
 * @example
 *
 * RA.isSymbol(Symbol('1')); //=> true
 * RA.isSymbol(Symbol(1)); //=> true
 * RA.isSymbol('string'); //=> false
 * RA.isSymbol(undefined); //=> false
 * RA.isSymbol(null); //=> false
 */
var isSymbol = (0, _ramda.curryN)(1, function (val) {
  return _typeof(val) === 'symbol' || _typeof(val) === 'object' && (0, _ramda.type)(val) === 'Symbol';
});
var _default = exports["default"] = isSymbol;