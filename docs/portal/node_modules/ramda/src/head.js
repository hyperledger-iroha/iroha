var _curry1 = /*#__PURE__*/require("./internal/_curry1.js");
var _nth = /*#__PURE__*/require("./internal/_nth.js");
/**
 * Returns the first element of the given list or string. In some libraries
 * this function is named `first`.
 *
 * @func
 * @memberOf R
 * @since v0.1.0
 * @category List
 * @sig [a] -> a | Undefined
 * @sig String -> String | Undefined
 * @param {Array|String} list
 * @return {*}
 * @see R.tail, R.init, R.last
 * @example
 *
 *      R.head(['fi', 'fo', 'fum']); //=> 'fi'
 *      R.head([]); //=> undefined
 *
 *      R.head('abc'); //=> 'a'
 *      R.head(''); //=> undefined
 */
var head = /*#__PURE__*/_curry1(function (list) {
  return _nth(0, list);
});
module.exports = head;