"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _invokeArgs = _interopRequireDefault(require("./invokeArgs.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
/**
 * Checks if an object exists in another object's prototype chain.
 *
 * @func isPrototypeOf
 * @category Object
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/2.31.0|v2.31.0}
 * @sig * -> Boolean
 * @param {Object} type The prototype that we're searching for
 * @param {Object} object The object whose prototype chain will be searched
 * @return {boolean}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/isPrototypeOf|Object.prorotype.isPrototypeOf}
 * @example
 * function Foo() {}
 * function Bar() {}
 * function Baz() {}
 *
 * Bar.prototype = Object.create(Foo.prototype);
 * Baz.prototype = Object.create(Bar.prototype);
 *
 * const baz = new Baz();
 *
 * RA.isPrototypeOf(Baz, baz); // => true
 * RA.isPrototypeOf(Bar, baz); // => true
 * RA.isPrototypeOf(Foo, baz); // => true
 * RA.isPrototypeOf(Object, baz); // => true
 */
var isPrototypeOf = (0, _ramda.curry)(function (type, object) {
  return Boolean((0, _invokeArgs["default"])(['prototype', 'isPrototypeOf'], [object], type));
});
var _default = exports["default"] = isPrototypeOf;