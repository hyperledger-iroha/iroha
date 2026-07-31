"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isMap = _interopRequireDefault(require("./isMap.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is complement of `Map` object.
 *
 * @func isNotMap
 * @memberOf RA
 * @category Type
 * @since {@link https://char0n.github.io/ramda-adjunct/2.27.0|v2.27.0}
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isMap|isMap}
 * @example
 *
 * RA.isNotMap(new Map()); //=> false
 * RA.isNotMap(new Map([[1, 2], [2, 1]])); //=> false
 * RA.isNotMap(new Set()); //=> true
 * RA.isNotMap({}); //=> true
 * RA.isNotMap(12); //=> true
 */

var isNotMap = (0, _ramda.complement)(_isMap["default"]);
var _default = exports["default"] = isNotMap;