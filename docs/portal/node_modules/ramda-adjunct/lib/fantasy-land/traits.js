"use strict";

exports.__esModule = true;
exports.setoidTrait = exports.semigroupTrait = exports.ordTrait = exports.functorTrait = exports.chainTrait = exports.applyTrait = void 0;
var _ramda = require("ramda");
var _isString = _interopRequireDefault(require("../isString.js"));
var _isNumber = _interopRequireDefault(require("../isNumber.js"));
var _isFunction = _interopRequireDefault(require("../isFunction.js"));
var _util = require("./util.js");
var fl = _interopRequireWildcard(require("./mapping.js"));
function _getRequireWildcardCache(e) { if ("function" != typeof WeakMap) return null; var r = new WeakMap(), t = new WeakMap(); return (_getRequireWildcardCache = function _getRequireWildcardCache(e) { return e ? t : r; })(e); }
function _interopRequireWildcard(e, r) { if (!r && e && e.__esModule) return e; if (null === e || "object" != _typeof(e) && "function" != typeof e) return { "default": e }; var t = _getRequireWildcardCache(r); if (t && t.has(e)) return t.get(e); var n = { __proto__: null }, a = Object.defineProperty && Object.getOwnPropertyDescriptor; for (var u in e) if ("default" !== u && {}.hasOwnProperty.call(e, u)) { var i = a ? Object.getOwnPropertyDescriptor(e, u) : null; i && (i.get || i.set) ? Object.defineProperty(n, u, i) : n[u] = e[u]; } return n["default"] = e, t && t.set(e, n), n; }
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
var functorTrait = exports.functorTrait = _defineProperty({}, fl.map, function (fn) {
  return this.constructor[fl.of](fn(this.value));
});
var applyTrait = exports.applyTrait = _defineProperty({}, fl.ap, function (applyWithFn) {
  var _this = this;
  return applyWithFn.map(function (fn) {
    return fn(_this.value);
  });
});
var setoidTrait = exports.setoidTrait = _defineProperty({}, fl.equals, function (setoid) {
  return (0, _util.isSameType)(this, setoid) && (0, _ramda.equals)(this.value, setoid.value);
});
var semigroupTrait = exports.semigroupTrait = _defineProperty({}, fl.concat, function (semigroup) {
  var concatenatedValue = this.value;
  if ((0, _isString["default"])(this.value) || (0, _isNumber["default"])(this.value)) {
    concatenatedValue = this.value + semigroup.value;
  } else if ((0, _ramda.pathSatisfies)(_isFunction["default"], ['value', fl.concat], this)) {
    concatenatedValue = this.value[fl.concat](semigroup.value);
  } else if ((0, _ramda.pathSatisfies)(_isFunction["default"], ['value', 'concat'], this)) {
    concatenatedValue = this.value.concat(semigroup.value);
  }
  return this.constructor[fl.of](concatenatedValue);
});
var chainTrait = exports.chainTrait = _defineProperty({}, fl.chain, function (fn) {
  var newChain = fn(this.value);
  return (0, _util.isSameType)(this, newChain) ? newChain : this;
});
var ordTrait = exports.ordTrait = _defineProperty({}, fl.lte, function (ord) {
  return (0, _util.isSameType)(this, ord) && (this.value < ord.value || this[fl.equals](ord));
});