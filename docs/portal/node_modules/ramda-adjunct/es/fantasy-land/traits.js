function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
import { equals, pathSatisfies } from 'ramda';
import isString from '../isString.js';
import isNumber from '../isNumber.js';
import isFunction from '../isFunction.js';
import { isSameType } from './util.js';
import * as fl from './mapping.js';
export var functorTrait = _defineProperty({}, fl.map, function (fn) {
  return this.constructor[fl.of](fn(this.value));
});
export var applyTrait = _defineProperty({}, fl.ap, function (applyWithFn) {
  var _this = this;
  return applyWithFn.map(function (fn) {
    return fn(_this.value);
  });
});
export var setoidTrait = _defineProperty({}, fl.equals, function (setoid) {
  return isSameType(this, setoid) && equals(this.value, setoid.value);
});
export var semigroupTrait = _defineProperty({}, fl.concat, function (semigroup) {
  var concatenatedValue = this.value;
  if (isString(this.value) || isNumber(this.value)) {
    concatenatedValue = this.value + semigroup.value;
  } else if (pathSatisfies(isFunction, ['value', fl.concat], this)) {
    concatenatedValue = this.value[fl.concat](semigroup.value);
  } else if (pathSatisfies(isFunction, ['value', 'concat'], this)) {
    concatenatedValue = this.value.concat(semigroup.value);
  }
  return this.constructor[fl.of](concatenatedValue);
});
export var chainTrait = _defineProperty({}, fl.chain, function (fn) {
  var newChain = fn(this.value);
  return isSameType(this, newChain) ? newChain : this;
});
export var ordTrait = _defineProperty({}, fl.lte, function (ord) {
  return isSameType(this, ord) && (this.value < ord.value || this[fl.equals](ord));
});