"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _allP = _interopRequireDefault(require("./allP.js"));
var _rejectP = _interopRequireDefault(require("./rejectP.js"));
var _resolveP = _interopRequireDefault(require("./resolveP.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns a Promise that is resolved with an array of reasons when all of the provided Promises reject, or rejected when any Promise is resolved.
 * This pattern is like allP, but fulfillments and rejections are transposed - rejections become the fulfillment values and vice versa.
 *
 * @func noneP
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category Function
 * @sig [Promise a] -> Promise [a]
 * @param {Iterable.<*>} iterable An iterable object such as an Array or String
 * @return {Promise} A Promise that is resolved with a list of rejection reasons if all Promises are rejected, or a Promise that is rejected with the fulfillment value of the first Promise that resolves.
 * @see {@link RA.allP|allP}
 * @example
 *
 * RA.noneP([Promise.reject('hello'), Promise.reject('world')]); //=> Promise(['hello', 'world'])
 * RA.noneP([]); //=> Promise([])
 * RA.noneP([Promise.reject(), Promise.resolve('hello world')]); //=> Promise('hello world')
 * RA.noneP([Promise.reject(), 'hello world']); //=> Promise('hello world')
 */
var noneP = (0, _ramda.curryN)(1, (0, _ramda.pipe)((0, _ramda.map)(_resolveP["default"]), (0, _ramda.map)(function (p) {
  return p.then(_rejectP["default"], _resolveP["default"]);
}), _allP["default"]));
var _default = exports["default"] = noneP;