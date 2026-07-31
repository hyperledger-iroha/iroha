"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _invokeArgs = _interopRequireDefault(require("./invokeArgs.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Invokes the method at path of object.
 *
 * @func invoke
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.31.0|v2.31.0}
 * @category Object
 * @sig Array -> Object -> *
 * @param {Array.<string|number>} path The path of the method to invoke
 * @param {Object} obj The object to query
 * @return {*}
 * @example
 *
 * RA.invoke(['random'], Math); //=> 0.5113253820009047
 */
var invoke = (0, _invokeArgs["default"])(_ramda.__, [], _ramda.__);
var _default = exports["default"] = invoke;