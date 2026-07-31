"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isObj = _interopRequireDefault(require("./isObj.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/* eslint-disable max-len */
/**
 * Checks if input value is complement of language type of `Object`.
 *
 * @func isNotObj
 * @aliases isNotObject
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.5.0|v0.5.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isObj|isObj}, {@link RA.isObjLike|isObjLike}, {@link RA.isPlainObj|isPlainObj}
 * @example
 *
 * RA.isNotObj({}); //=> false
 * RA.isNotObj([]); //=> false
 * RA.isNotObj(() => {}); //=> false
 * RA.isNotObj(null); //=> true
 * RA.isNotObj(undefined); //=> true
 */
/* eslint-enable max-len */
var isNotObj = (0, _ramda.complement)(_isObj["default"]);
var _default = exports["default"] = isNotObj;