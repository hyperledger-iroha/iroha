"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Returns a "view" of the given data structure, determined by the given lens.
 * The lens's focus determines which portion of the data structure is visible.
 * Returns the defaultValue if "view" is null, undefined or NaN; otherwise the "view" is returned.
 *
 * @func viewOr
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.13.0|1.13.0}
 * @category Object
 * @typedef Lens s b = Functor f => (b -> f b) -> s -> f s
 * @sig a -> Lens s b -> s -> b | a
 * @see {@link http://ramdajs.com/docs/#view|R.view}
 * @param {*} defaultValue The default value
 * @param {Function} lens Van Laarhoven lens
 * @param {*} data The data structure
 * @returns {*} "view" or defaultValue
 *
 * @example
 *
 * RA.viewOr('N/A', R.lensProp('x'), {}); // => 'N/A'
 * RA.viewOr('N/A', R.lensProp('x'), { x: 1 }); // => 1
 * RA.viewOr('some', R.lensProp('y'), { y: null }); // => 'some'
 * RA.viewOr('some', R.lensProp('y'), { y: false }); // => false
 */

var viewOr = (0, _ramda.curryN)(3, function (defaultValue, lens, data) {
  return (0, _ramda.defaultTo)(defaultValue, (0, _ramda.view)(lens, data));
});
var _default = exports["default"] = viewOr;