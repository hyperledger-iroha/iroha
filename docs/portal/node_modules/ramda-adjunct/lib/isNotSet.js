"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isSet = _interopRequireDefault(require("./isSet.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if value is complement of `Set` object.
 *
 * @func isNotSet
 * @memberOf RA
 * @category Type
 * @since {@link https://char0n.github.io/ramda-adjunct/2.27.0|v2.27.0}
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isSet|isSet}
 * @example
 *
 * RA.isNotSet(new Map()); //=> true
 * RA.isNotSet(new Set()); //=> false
 * RA.isNotSet(new Set([1,2]); //=> false
 * RA.isNotSet(new Object()); //=> true
 */

var isNotSet = (0, _ramda.complement)(_isSet["default"]);
var _default = exports["default"] = isNotSet;