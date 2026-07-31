"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNotArray = _interopRequireDefault(require("./isNotArray.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Returns a singleton array containing the value provided.
 * If value is already an array, it is returned as is.
 *
 * @func ensureArray
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.6.0|v2.6.0}
 * @category List
 * @sig a | [a] -> [a]
 * @param {*|Array} val the value ensure as Array
 * @return {Array}
 * @see {@link http://ramdajs.com/docs/#of|R.of}
 * @example
 *
 * RA.ensureArray(42); //=> [42]
 * RA.ensureArray([42]); //=> [42]
 */
var ensureArray = (0, _ramda.when)(_isNotArray["default"], (0, _ramda.of)(Array));
var _default = exports["default"] = ensureArray;