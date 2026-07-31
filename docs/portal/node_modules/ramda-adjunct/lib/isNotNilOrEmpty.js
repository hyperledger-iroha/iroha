"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNilOrEmpty = _interopRequireDefault(require("./isNilOrEmpty.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns `false` if the given value is its type's empty value, `null` or `undefined`.
 *
 * @func isNotNilOrEmpty
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.18.0|v2.18.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNilOrEmpty|isNilOrEmpty}
 * @example
 *
 * RA.isNotNilOrEmpty([1, 2, 3]); //=> true
 * RA.isNotNilOrEmpty([]); //=> false
 * RA.isNotNilOrEmpty(''); //=> false
 * RA.isNotNilOrEmpty(null); //=> false
 * RA.isNotNilOrEmpty(undefined): //=> false
 * RA.isNotNilOrEmpty({}); //=> false
 * RA.isNotNilOrEmpty({length: 0}); //=> true
 */
var isNotNilOrEmpty = (0, _ramda.complement)(_isNilOrEmpty["default"]);
var _default = exports["default"] = isNotNilOrEmpty;