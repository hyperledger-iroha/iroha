function _toArray(r) { return _arrayWithHoles(r) || _iterableToArray(r) || _unsupportedIterableToArray(r) || _nonIterableRest(); }
function _nonIterableRest() { throw new TypeError("Invalid attempt to destructure non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
function _iterableToArray(r) { if ("undefined" != typeof Symbol && null != r[Symbol.iterator] || null != r["@@iterator"]) return Array.from(r); }
function _arrayWithHoles(r) { if (Array.isArray(r)) return r; }
import { comparator, curry, either, lt, map, prop, reduce, sort } from 'ramda';

/**
 * Sort a list of objects by a list of props (if first prop value is equivalent, sort by second, etc).
 *
 * @func sortByProps
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.26.0|v2.26.0}
 * @category List
 * @sig [k] -> [{k: v}] -> [{k: v}]
 * @param {Array.<string>} props A list of properties in the list param to sort by
 * @param {Array.<object>} list A list of objects to be sorted
 * @return {Array.<object>} A new list sorted by the properties in the props param
 * @example
 *
 * sortByProps(['num'], [{num: 3}, {num: 2}, {num: 1}])
 * //=> [{num: 1}, {num: 2} {num: 3}]
 * sortByProps(['letter', 'num'], [{num: 3, letter: 'a'}, {num: 2, letter: 'a'} {num: 1, letter: 'z'}])
 * //=> [ {num: 2, letter: 'a'}, {num: 3, letter: 'a'}, {num: 1, letter: 'z'}]
 * sortByProps(['name', 'num'], [{num: 3}, {num: 2}, {num: 1}])
 * //=> [{num: 1}, {num: 2}, {num: 3}]
 */

var sortByProps = curry(function (props, list) {
  var firstTruthy = function firstTruthy(_ref) {
    var _ref2 = _toArray(_ref),
      head = _ref2[0],
      tail = _ref2.slice(1);
    return reduce(either, head, tail);
  };
  var makeComparator = function makeComparator(propName) {
    return comparator(function (a, b) {
      return lt(prop(propName, a), prop(propName, b));
    });
  };
  return sort(firstTruthy(map(makeComparator, props)), list);
});
export default sortByProps;