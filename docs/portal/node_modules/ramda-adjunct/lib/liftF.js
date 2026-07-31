"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _liftFN = _interopRequireDefault(require("./liftFN.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * "lifts" a function to be the specified arity, so that it may "map over" objects that satisfy
 * the fantasy land Apply spec of algebraic structures.
 *
 * Lifting is specific for {@link https://github.com/scalaz/scalaz|scalaz} and {@link http://functionaljava.org/|function Java} implementations.
 * Old version of fantasy land spec were not compatible with this approach,
 * but as of fantasy land 1.0.0 Apply spec also adopted this approach.
 *
 * This function acts as interop for ramda <= 0.23.0 and {@link https://monet.github.io/monet.js/|monet.js}.
 *
 * More info {@link https://github.com/fantasyland/fantasy-land/issues/50|here}.
 *
 * @func liftF
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.2.0|v1.2.0}
 * @category Function
 * @sig Apply a => (a... -> a) -> (a... -> a)
 * @param {Function} fn The function to lift into higher context
 * @return {Function} The lifted function
 * @see {@link RA.liftFN|liftFN}
 * @example
 *
 * const { Maybe } = require('monet');
 *
 * const add3 = (a, b, c) => a + b + c;
 * const madd3 = RA.liftF(add3);
 *
 * madd3(Maybe.Some(10), Maybe.Some(15), Maybe.Some(17)); //=> Maybe.Some(42)
 * madd3(Maybe.Some(10), Maybe.Nothing(), Maybe.Some(17)); //=> Maybe.Nothing()
 */
var liftF = (0, _ramda.curryN)(1, function (fn) {
  return (0, _liftFN["default"])(fn.length, fn);
});
var _default = exports["default"] = liftF;