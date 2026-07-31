function _toConsumableArray(r) { return _arrayWithoutHoles(r) || _iterableToArray(r) || _unsupportedIterableToArray(r) || _nonIterableSpread(); }
function _nonIterableSpread() { throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _iterableToArray(r) { if ("undefined" != typeof Symbol && null != r[Symbol.iterator] || null != r["@@iterator"]) return Array.from(r); }
function _arrayWithoutHoles(r) { if (Array.isArray(r)) return _arrayLikeToArray(r); }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
import { sort, comparator, prop, pipe, head, curryN, reduce, reduced, curry, ifElse } from 'ramda';

/**
 * Can be used as a way to compose multiple invokers together to form polymorphic functions,
 * or functions that exhibit different behaviors based on their argument(s).
 * Consumes dispatching functions and keep trying to invoke each in turn, until a non-nil value is returned.
 *
 * Accepts a list of dispatching functions and returns a new function.
 * When invoked, this new function is applied to some arguments,
 * each dispatching function is applied to those same arguments until one of the
 * dispatching functions returns a non-nil value.
 *
 * @func dispatch
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.6.0|v2.6.0}
 * @category Function
 * @sig [((a, b, ...) -> x1), ((a, b, ...) -> x2), ...] -> x1 | x2 | ...
 * @param {!Array} functions A list of functions
 * @return {*|undefined} Returns the first not-nil value, or undefined if either an empty list is provided or none of the dispatching functions returns a non-nil value
 * @see {@link RA.isNotNil}
 * @example
 *
 * // returns first non-nil value
 * const stubNil = () => null;
 * const stubUndefined = () => undefined;
 * const addOne = v => v + 1;
 * const addTwo = v => v + 2;
 *
 * RA.dispatch([stubNil, stubUndefined, addOne, addTwo])(1); //=> 2
 *
 * // acts as a switch
 * const fnSwitch = RA.dispatch([
 *   R.ifElse(RA.isString, s => `${s}-join`, RA.stubUndefined),
 *   R.ifElse(RA.isNumber, n => n + 1, RA.stubUndefined),
 *   R.ifElse(RA.isDate, R.T, RA.stubUndefined),
 * ]);
 * fnSwitch(1); //=> 2
 */
import isNotNil from './isNotNil.js';
import isNonEmptyArray from './isNonEmptyArray.js';
import stubUndefined from './stubUndefined.js';
var byArity = comparator(function (a, b) {
  return a.length > b.length;
});
var getMaxArity = pipe(sort(byArity), head, prop('length'));
var iteratorFn = curry(function (args, accumulator, fn) {
  var result = fn.apply(void 0, _toConsumableArray(args));
  return isNotNil(result) ? reduced(result) : accumulator;
});
var dispatchImpl = function dispatchImpl(functions) {
  var arity = getMaxArity(functions);
  return curryN(arity, function () {
    for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
      args[_key] = arguments[_key];
    }
    return reduce(iteratorFn(args), undefined, functions);
  });
};
var dispatch = ifElse(isNonEmptyArray, dispatchImpl, stubUndefined);
export default dispatch;