"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _defaultWhen = _interopRequireDefault(require("./defaultWhen.js"));
var _mapIndexed = _interopRequireDefault(require("./mapIndexed.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns a function which is called with the given arguments. If any of the given arguments are null or undefined,
 * the corresponding default value for that argument is used instead.
 *
 * @func fnull
 * @memberOf RA
 * @category Function
 * @sig (a ... -> b) -> [c] -> a ... | c -> b
 * @param {Function} function to be executed
 * @param {Array} defaults default arguments
 * @return {Function} will apply provided arguments or default ones
 * @example
 *
 * const addDefaults = RA.fnull((a, b) => a + b, [4, 5])
 *
 * addDefaults(1, 2); // => 3
 * addDefaults(null, 2); // => 6
 * addDefaults(2, null); // => 7
 * addDefaults(undefined, undefined); // => 9
 */

var fnull = (0, _ramda.curry)(function (fn, defaults) {
  return (0, _ramda.curryN)(fn.length, function () {
    for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
      args[_key] = arguments[_key];
    }
    var argsWithDefaults = (0, _mapIndexed["default"])(function (val, idx) {
      return (0, _defaultWhen["default"])(_ramda.isNil, defaults[idx], val);
    }, args);
    return (0, _ramda.apply)(fn, argsWithDefaults);
  });
});
var _default = exports["default"] = fnull;