var _curry1 = /*#__PURE__*/require("./internal/_curry1.js");
var _nth = /*#__PURE__*/require("./internal/_nth.js");
/**
 * Returns the last element of the given list or string.
 *
 * @func
 * @memberOf R
 * @since v0.1.4
 * @category List
 * @sig [a] -> a | Undefined
 * @sig String -> String | Undefined
 * @param {*} list
 * @return {*}
 * @see R.init, R.head, R.tail
 * @example
 *
 *      R.last(['fi', 'fo', 'fum']); //=> 'fum'
 *      R.last([]); //=> undefined
 *
 *      R.last('abc'); //=> 'c'
 *      R.last(''); //=> undefined
 */
var last = /*#__PURE__*/_curry1(function (list) {
  return _nth(-1, list);
});
module.exports = last;