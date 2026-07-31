"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isUndefined = _interopRequireDefault(require("./isUndefined.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if input value is complement `undefined`.
 *
 * @func isNotUndefined
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.0.1|v0.0.1}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isUndefined|isUndefined}
 * @example
 *
 * RA.isNotUndefined(1); //=> true
 * RA.isNotUndefined(undefined); //=> false
 * RA.isNotUndefined(null); //=> true
 */
var isNotUndefined = (0, _ramda.complement)(_isUndefined["default"]);
var _default = exports["default"] = isNotUndefined;