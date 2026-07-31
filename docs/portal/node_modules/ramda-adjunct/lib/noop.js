"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _stubUndefined = _interopRequireDefault(require("./stubUndefined.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * A function that performs no operations.
 *
 * @func noop
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.0.0|v1.0.0}
 * @category Function
 * @sig ... -> undefined
 * @return {undefined}
 * @example
 *
 * RA.noop(); //=> undefined
 * RA.noop(1, 2, 3); //=> undefined
 */
var noop = (0, _ramda.always)((0, _stubUndefined["default"])());
var _default = exports["default"] = noop;