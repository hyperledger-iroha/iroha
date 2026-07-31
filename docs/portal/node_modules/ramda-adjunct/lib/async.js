"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _resolveP = _interopRequireDefault(require("./resolveP.js"));
var _rejectP = _interopRequireDefault(require("./rejectP.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Takes a generator function and returns an async function.
 * The async function returned is a curried function whose arity matches that of the generator function.
 *
 * Note: This function is handy for environments that does support generators but doesn't support async/await.
 *
 * @func async
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.16.0|v2.16.0}
 * @category Function
 * @sig Promise c => (a, b, ...) -> a -> b -> ... -> c
 * @param {Function} generatorFn The generator function
 * @return {Function} Curried async function
 * @see {@link https://www.promisejs.org/generators/}
 * @example
 *
 * const asyncFn = RA.async(function* generator(val1, val2) {
 *   const a = yield Promise.resolve(val1);
 *   const b = yield Promise.resolve(val2);
 *
 *   return a + b;
 * });
 *
 * asyncFn(1, 2); //=> Promise(3)
 *
 */
var async = (0, _ramda.curryN)(1, function (generatorFn) {
  function asyncWrapper() {
    var iterator = (0, _ramda.bind)(generatorFn, this).apply(void 0, arguments);
    var _handle = function handle(result) {
      var resolved = (0, _resolveP["default"])(result.value);
      return result.done ? resolved : resolved.then(function (value) {
        return _handle(iterator.next(value));
      }, function (error) {
        return _handle(iterator["throw"](error));
      });
    };
    try {
      return _handle(iterator.next());
    } catch (error) {
      return (0, _rejectP["default"])(error);
    }
  }
  if (generatorFn.length > 0) {
    return (0, _ramda.curryN)(generatorFn.length, asyncWrapper);
  }
  return asyncWrapper;
});
var _default = exports["default"] = async;