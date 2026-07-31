"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNotFunction = _interopRequireDefault(require("./isNotFunction.js"));
var _isEmptyArray = _interopRequireDefault(require("./isEmptyArray.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Invokes the method at path of object with given arguments.
 *
 * @func invokeArgs
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.27.0|v2.27.0}
 * @category Object
 * @sig Array -> Array -> Object -> *
 * @param {Array.<string|number>} path The path of the method to invoke
 * @param {Array} args The arguments to invoke the method with
 * @param {Object} obj The object to query
 * @return {*}
 * @example
 *
 * RA.invokeArgs(['abs'], [-1], Math); //=> 1
 * RA.invokeArgs(['path', 'to', 'non-existent', 'method'], [-1], Math); //=> undefined
 */

var invokeArgs = (0, _ramda.curryN)(3, function (mpath, args, obj) {
  var method = (0, _ramda.path)(mpath, obj);
  var context = (0, _ramda.path)((0, _ramda.init)(mpath), obj);
  if ((0, _isNotFunction["default"])(method)) return undefined;
  if ((0, _isEmptyArray["default"])(mpath)) return undefined;
  var boundMethod = (0, _ramda.bind)(method, context);
  return (0, _ramda.apply)(boundMethod, args);
});
var _default = exports["default"] = invokeArgs;