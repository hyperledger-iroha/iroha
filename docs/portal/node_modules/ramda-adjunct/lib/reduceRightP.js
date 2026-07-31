"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isUndefined = _interopRequireDefault(require("./isUndefined.js"));
var _resolveP = _interopRequireDefault(require("./resolveP.js"));
var _allP = _interopRequireDefault(require("./allP.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _slicedToArray(r, e) { return _arrayWithHoles(r) || _iterableToArrayLimit(r, e) || _unsupportedIterableToArray(r, e) || _nonIterableRest(); }
function _nonIterableRest() { throw new TypeError("Invalid attempt to destructure non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
function _iterableToArrayLimit(r, l) { var t = null == r ? null : "undefined" != typeof Symbol && r[Symbol.iterator] || r["@@iterator"]; if (null != t) { var e, n, i, u, a = [], f = !0, o = !1; try { if (i = (t = t.call(r)).next, 0 === l) { if (Object(t) !== t) return; f = !1; } else for (; !(f = (e = i.call(t)).done) && (a.push(e.value), a.length !== l); f = !0); } catch (r) { o = !0, n = r; } finally { try { if (!f && null != t["return"] && (u = t["return"](), Object(u) !== u)) return; } finally { if (o) throw n; } } return a; } }
function _arrayWithHoles(r) { if (Array.isArray(r)) return r; }
// in older ramda versions the order of the arguments is flipped
var flipArgs = (0, _ramda.pipe)((0, _ramda.reduceRight)(_ramda.concat, ''), (0, _ramda.equals)('ba'))(['a', 'b']);

/* eslint-disable max-len */
/**
 * Given an `Iterable`(arrays are `Iterable`), or a promise of an `Iterable`,
 * which produces promises (or a mix of promises and values),
 * iterate over all the values in the `Iterable` into an array and
 * reduce the array to a value using the given iterator function.
 *
 * Similar to {@link RA.reduceP|reduceP} except moves through the input list from the right to the left.
 * The iterator function receives two values: (value, acc),
 * while the arguments' order of reduceP's iterator function is (acc, value).
 *
 * @func reduceRightP
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.13.0|v1.13.0}
 * @category List
 * @typedef MaybePromise = Promise.<*> | *
 * @sig ((MaybePromise b, Promise a) -> Promise a) -> MaybePromise a -> MaybePromise [MaybePromise b] -> Promise a
 * @param {Function} fn The iterator function. Receives two values, the current element from the list and the accumulator
 * @param {*|Promise.<*>} acc The accumulator value
 * @param {Array.<*>|Promise.<Array<*|Promise.<*>>>} list The list to iterate over
 * @return {Promise} The final, accumulated value
 * @see {@link RA.reduceP|reduceP}, {@link http://bluebirdjs.com/docs/api/promise.reduce.html|bluebird.reduce}
 * @example
 *
 * RA.reduceRightP(
 *   (fileName, total) => fs
 *     .readFileAsync(fileName, 'utf8')
 *     .then(contents => total + parseInt(contents, 10)),
 *   0,
 *   ['file1.txt', 'file2.txt', 'file3.txt']
 * ); // => Promise(10)
 *
 * RA.reduceRightP(
 *   (fileName, total) => fs
 *     .readFileAsync(fileName, 'utf8')
 *     .then(contents => total + parseInt(contents, 10)),
 *   Promise.resolve(0),
 *   ['file1.txt', 'file2.txt', 'file3.txt']
 * ); // => Promise(10)
 *
 * RA.reduceRightP(
 *   (fileName, total) => fs
 *     .readFileAsync(fileName, 'utf8')
 *     .then(contents => total + parseInt(contents, 10)),
 *   0,
 *   [Promise.resolve('file1.txt'), 'file2.txt', 'file3.txt']
 * ); // => Promise(10)
 *
 * RA.reduceRightP(
 *   (fileName, total) => fs
 *     .readFileAsync(fileName, 'utf8')
 *     .then(contents => total + parseInt(contents, 10)),
 *   0,
 *   Promise.resolve([Promise.resolve('file1.txt'), 'file2.txt', 'file3.txt'])
 * ); // => Promise(10)
 *
 */
/* esline-enable max-len */
var reduceRightP = (0, _ramda.curryN)(3, function (fn, acc, list) {
  return (0, _resolveP["default"])(list).then(function (iterable) {
    var listLength = (0, _ramda.length)(iterable);
    if (listLength === 0) {
      return acc;
    }
    var reducer = (0, _ramda.reduceRight)(function (arg1, arg2) {
      var accP;
      var currentValueP;
      if (flipArgs) {
        accP = arg1;
        currentValueP = arg2;
      } else {
        accP = arg2;
        currentValueP = arg1;
      }
      return accP.then(function (previousValue) {
        return (0, _allP["default"])([previousValue, currentValueP]);
      }).then(function (_ref) {
        var _ref2 = _slicedToArray(_ref, 2),
          previousValue = _ref2[0],
          currentValue = _ref2[1];
        if ((0, _isUndefined["default"])(previousValue) && listLength === 1) {
          return currentValue;
        }
        return fn(currentValue, previousValue);
      });
    });
    return reducer((0, _resolveP["default"])(acc), iterable);
  });
});
var _default = exports["default"] = reduceRightP;