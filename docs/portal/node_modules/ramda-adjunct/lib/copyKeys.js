"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _renameKeys = _interopRequireDefault(require("./renameKeys.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function ownKeys(e, r) { var t = Object.keys(e); if (Object.getOwnPropertySymbols) { var o = Object.getOwnPropertySymbols(e); r && (o = o.filter(function (r) { return Object.getOwnPropertyDescriptor(e, r).enumerable; })), t.push.apply(t, o); } return t; }
function _objectSpread(e) { for (var r = 1; r < arguments.length; r++) { var t = null != arguments[r] ? arguments[r] : {}; r % 2 ? ownKeys(Object(t), !0).forEach(function (r) { _defineProperty(e, r, t[r]); }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(t)) : ownKeys(Object(t)).forEach(function (r) { Object.defineProperty(e, r, Object.getOwnPropertyDescriptor(t, r)); }); } return e; }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
/**
 * Creates a new object with the own properties of the provided object, and the
 * keys copied according to the keysMap object as `{oldKey: newKey}`.
 * When no key from the keysMap is found, then a shallow clone of an object is returned.
 *
 * Keep in mind that in the case of keys conflict is behaviour undefined and
 * the result may vary between various JS engines!
 *
 * @func copyKeys
 * @memberOf RA
 * @category Object
 * @sig {a: b} -> {a: *} -> {b: *}
 * @param {!Object} keysMap
 * @param {!Object} obj
 * @return {!Object} New object with copied keys
 * @see {@link RA.renameKeys|renameKeys}
 * @example
 *
 * copyKeys({ a: 'b' }, { a: true }); //=> { a: true, b: true }
 * copyKeys({ a: 'b' }, { a: true, b: false }); //=> { a: true, b: true }
 */
var copyKeys = (0, _ramda.curryN)(2, function (keysMap, obj) {
  return _objectSpread(_objectSpread({}, obj), (0, _renameKeys["default"])(keysMap, obj));
});
var _default = exports["default"] = copyKeys;