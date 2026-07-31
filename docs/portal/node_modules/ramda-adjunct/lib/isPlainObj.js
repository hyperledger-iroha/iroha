"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNull = _interopRequireDefault(require("./isNull.js"));
var _isObjLike = _interopRequireDefault(require("./isObjLike.js"));
var _isFunction = _interopRequireDefault(require("./isFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isObject = (0, _ramda.pipe)(_ramda.type, (0, _ramda.identical)('Object'));
var isObjectConstructor = (0, _ramda.pipe)(_ramda.toString, (0, _ramda.equals)((0, _ramda.toString)(Object)));
var hasObjectConstructor = (0, _ramda.pathSatisfies)((0, _ramda.both)(_isFunction["default"], isObjectConstructor), ['constructor']);

/* eslint-disable max-len */
/**
 * Check to see if an object is a plain object (created using `{}`, `new Object()` or `Object.create(null)`).
 *
 * @func isPlainObj
 * @aliases isPlainObject
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/0.5.0|v0.5.0}
 * @category Type
 * @sig * -> Boolean
 * @param {*} val The value to test
 * @return {boolean}
 * @see {@link RA.isNotPlainObj|isNotPlainObj}, {@link RA.isObjLike|isObjLike}, {@link RA.isObj|isObj}
 * @example
 *
 * class Bar {
 *   constructor() {
 *     this.prop = 'value';
 *   }
 * }
 *
 * RA.isPlainObj(new Bar()); //=> false
 * RA.isPlainObj({ prop: 'value' }); //=> true
 * RA.isPlainObj(['a', 'b', 'c']); //=> false
 * RA.isPlainObj(Object.create(null); //=> true
 * RA.isPlainObj(new Object()); //=> true
 */
/* eslint-enable max-len */
var isPlainObj = (0, _ramda.curryN)(1, function (val) {
  if (!(0, _isObjLike["default"])(val) || !isObject(val)) {
    return false;
  }
  var proto = Object.getPrototypeOf(val);
  if ((0, _isNull["default"])(proto)) {
    return true;
  }
  return hasObjectConstructor(proto);
});
var _default = exports["default"] = isPlainObj;