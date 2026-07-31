"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
/**
 * Returns true if both arguments are falsy; false otherwise.
 *
 * @func nor
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.20.0|v2.20.0}
 * @category Logic
 * @sig a -> b -> a ⊽ b
 * @param {*} a
 * @param {*} b
 * @return {boolean} true if both arguments are falsy
 * @see {@link RA.neither|neither}
 * @example
 *
 * RA.nor(true, true); //=> false
 * RA.nor(false, true); //=> false
 * RA.nor(true, false); //=> false
 * RA.nor(false, false); //=> true
 * RA.nor(1, 1); //=> false
 * RA.nor(1, 0); //=> false
 * RA.nor(0, 1); //=> false
 * RA.nor(0, 0); //=> true
 */
var nor = (0, _ramda.complement)(_ramda.or); // eslint-disable-line ramda/complement-simplification
var _default = exports["default"] = nor;