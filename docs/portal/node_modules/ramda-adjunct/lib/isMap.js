"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Predicate for determining if a provided value is an instance of a Map.
 *
 * @func isMap
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.22.0|v2.22.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isSet|isSet}}
 * @example
 *
 * RA.isMap(new Map()); //=> true
 * RA.isMap(new Map([[1, 2], [2, 1]])); //=> true
 * RA.isSet(new Set()); //=> false
 * RA.isSet(new Set([1,2]); //=> false
 * RA.isSet(new Object()); //=> false
 */

var isMap = (0, _ramda.curryN)(1, (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Map')));
var _default = exports["default"] = isMap;