"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks whether the passed value is iterable.
 *
 * @func isIterable
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.18.0|v2.18.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Iteration_protocols#The_iterable_protocol}
 * @return {boolean}
 * @example
 *
 * RA.isIterable(['arrays', 'are', 'iterable']); //=> true
 * RA.isIterable('strings are iterable, too'); //=> true
 * RA.isIterable((function* () {})()); //=> true (generator objects are both iterable and iterators)
 *
 * RA.isIterable({}); //=> false
 * RA.isIterable(-0); //=> false
 * RA.isIterable(null); //=> false
 * RA.isIterable(undefined); //=> false
 */
var isIterable = (0, _ramda.curryN)(1, function (val) {
  if (typeof Symbol === 'undefined') {
    return false;
  }
  return (0, _ramda.hasIn)(Symbol.iterator, Object(val)) && (0, _isFunction["default"])(val[Symbol.iterator]);
});
var _default = exports["default"] = isIterable;