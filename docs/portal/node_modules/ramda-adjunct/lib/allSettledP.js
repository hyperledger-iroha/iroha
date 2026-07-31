"use strict";

exports.__esModule = true;
exports["default"] = exports.allSettledPPonyfill = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
var _PromiseAllSettled = _interopRequireDefault(require("./internal/ponyfills/Promise.allSettled.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var allSettledPPonyfill = exports.allSettledPPonyfill = (0, _ramda.curryN)(1, _PromiseAllSettled["default"]);

/**
 * Returns a promise that is fulfilled with an array of promise state snapshots,
 * but only after all the original promises have settled, i.e. become either fulfilled or rejected.
 * We say that a promise is settled if it is not pending, i.e. if it is either fulfilled or rejected.
 *
 * @func allSettledP
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.18.0|v2.18.0}
 * @category Function
 * @typedef Settlement = { status: String, value: * }
 * @sig [Promise a] -> Promise [Settlement a]
 * @param {Iterable.<*>} iterable An iterable object such as an Array or String
 * @return {Promise} Returns a promise that is fulfilled with an array of promise state snapshots
 * @see {@link RA.allP|allP}
 * @example
 *
 * RA.allSettledP([
 *   Promise.resolve(1),
 *   2,
 *   Promise.reject(3),
 * ]); //=> Promise([{ status: 'fulfilled', value: 1 }, { status: 'fulfilled', value: 2 }, { status: 'rejected', reason: 3 }])
 */
var allSettledP = (0, _isFunction["default"])(Promise.allSettled) ? (0, _ramda.curryN)(1, (0, _ramda.bind)(Promise.allSettled, Promise)) : allSettledPPonyfill;
var _default = exports["default"] = allSettledP;