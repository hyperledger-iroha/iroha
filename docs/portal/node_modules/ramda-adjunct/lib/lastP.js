"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _allP = _interopRequireDefault(require("./allP.js"));
var _lengthEq = _interopRequireDefault(require("./lengthEq.js"));
var _lengthGte = _interopRequireDefault(require("./lengthGte.js"));
var _rejectP = _interopRequireDefault(require("./rejectP.js"));
var _resolveP = _interopRequireDefault(require("./resolveP.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _toConsumableArray(r) { return _arrayWithoutHoles(r) || _iterableToArray(r) || _unsupportedIterableToArray(r) || _nonIterableSpread(); }
function _nonIterableSpread() { throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _iterableToArray(r) { if ("undefined" != typeof Symbol && null != r[Symbol.iterator] || null != r["@@iterator"]) return Array.from(r); }
function _arrayWithoutHoles(r) { if (Array.isArray(r)) return _arrayLikeToArray(r); }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
/**
 * Returns a promise that is fulfilled by the last given promise to be fulfilled,
 * or rejected with an array of rejection reasons if all of the given promises are rejected.
 *
 * @func lastP
 * @memberOf RA
 * @category Function
 * @since {@link https://char0n.github.io/ramda-adjunct/2.23.0|v2.23.0}
 * @sig [Promise a] -> Promise a
 * @param {Iterable.<*>} iterable An iterable object such as an Array or String
 * @return {Promise} A promise that is fulfilled by the last given promise to be fulfilled, or rejected with an array of rejection reasons if all of the given promises are rejected.
 * @see {@link RA.anyP|anyP}
 * @example
 *
 * const delayP = timeout => new Promise(resolve => setTimeout(() => resolve(timeout), timeout));
 * delayP.reject = timeout => new Promise((resolve, reject) => setTimeout(() => reject(timeout), timeout));
 * RA.lastP([
 *   1,
 *   delayP(10),
 *   delayP(100),
 *   delayP.reject(1000),
 * ]); //=> Promise(100)
 */
var lastP = (0, _ramda.curryN)(1, function (iterable) {
  var fulfilled = [];
  var rejected = [];
  var onFulfill = (0, _ramda.bind)(fulfilled.push, fulfilled);
  var onReject = (0, _ramda.bind)(rejected.push, rejected);
  var listOfPromises = (0, _ramda.map)(function (p) {
    return (0, _resolveP["default"])(p).then(onFulfill)["catch"](onReject);
  }, _toConsumableArray(iterable));
  return (0, _allP["default"])(listOfPromises).then(function () {
    if ((0, _lengthEq["default"])(0, fulfilled) && (0, _lengthEq["default"])(0, rejected)) {
      return undefined;
    }
    if ((0, _lengthGte["default"])(1, fulfilled)) {
      return (0, _ramda.last)(fulfilled);
    }
    return (0, _rejectP["default"])(rejected);
  });
});
var _default = exports["default"] = lastP;