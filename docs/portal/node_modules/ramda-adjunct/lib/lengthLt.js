"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _compareLength = _interopRequireDefault(require("./internal/compareLength.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns `true` if the supplied list or string has a length less than `valueLength`.
 *
 * @func lengthLt
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.8.0|v2.8.0}
 * @category List
 * @sig Number -> [*] -> Boolean
 * @param {number} valueLength The length of the list or string
 * @param {Array|string} value The list or string
 * @return {boolean}
 * @see {@link RA.lengthEq|lengthEq}, {@link RA.lengthNotEq|lengthNotEq}, {@link RA.lengthGt|lengthGt}, {@link RA.lengthLte|lengthLte}, {@link RA.lengthGte|lengthGte}, {@link http://ramdajs.com/docs/#lt|lt}, {@link http://ramdajs.com/docs/#length|length}
 * @example
 *
 * RA.lengthLt(3, [1,2]); //=> true
 * RA.lengthLt(3, [1,2,3]); //=> false
 */
var lengthLt = (0, _compareLength["default"])((0, _ramda.flip)(_ramda.lt));
var _default = exports["default"] = lengthLt;