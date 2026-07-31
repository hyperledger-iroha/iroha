"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFalsy = _interopRequireDefault(require("./isFalsy.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Creates an array with all falsy values removed.
 * The values false, null, 0, "", undefined, and NaN are falsy.
 *
 * @func compact
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.5.0|v2.5.0}
 * @category List
 * @sig Filterable f => f a -> f a
 * @param {Array} list The array to compact
 * @return {Array} Returns the new array of filtered values
 * @see {@link RA.isFalsy|isFalsy}
 * @example
 *
 * RA.compact([0, 1, false, 2, '', 3]); //=> [1, 2, 3]
 */
var compact = (0, _ramda.reject)(_isFalsy["default"]);
var _default = exports["default"] = compact;