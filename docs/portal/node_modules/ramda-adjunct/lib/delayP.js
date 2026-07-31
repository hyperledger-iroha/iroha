"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNonNegative = _interopRequireDefault(require("./isNonNegative.js"));
var _isInteger = _interopRequireDefault(require("./isInteger.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Creates a promise which resolves/rejects after the specified milliseconds.
 *
 * @func delayP
 * @memberOf RA
 * @category Function
 * @sig Number -> Promise Undefined
 * @sig {timeout: Number, value: a} -> Promise a
 * @param {number|Object} milliseconds number of milliseconds or options object
 * @return {Promise} A Promise that is resolved/rejected with the given value (if provided) after the specified delay
 * @example
 *
 * RA.delayP(200); //=> Promise(undefined)
 * RA.delayP({ timeout: 1000, value: 'hello world' }); //=> Promise('hello world')
 * RA.delayP.reject(100); //=> Promise(undefined)
 * RA.delayP.reject({ timeout: 100, value: new Error('error') }); //=> Promise(Error('error'))
 */

var makeDelay = (0, _ramda.curry)(function (settleFnPicker, opts) {
  var timeout;
  var value;
  if ((0, _isInteger["default"])(opts) && (0, _isNonNegative["default"])(opts)) {
    timeout = opts;
  } else {
    timeout = (0, _ramda.propOr)(0, 'timeout', opts);
    value = (0, _ramda.propOr)(value, 'value', opts);
  }
  return new Promise(function () {
    for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
      args[_key] = arguments[_key];
    }
    var settleFn = settleFnPicker(args);
    setTimeout((0, _ramda.partial)(settleFn, [value]), timeout);
  });
});
var delayP = makeDelay((0, _ramda.nth)(0));
delayP.reject = makeDelay((0, _ramda.nth)(1));
var _default = exports["default"] = delayP;