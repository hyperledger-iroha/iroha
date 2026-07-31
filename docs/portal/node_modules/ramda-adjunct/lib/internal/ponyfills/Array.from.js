"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isIterable = _interopRequireDefault(require("../../isIterable.js"));
var _isNotUndefined = _interopRequireDefault(require("../../isNotUndefined.js"));
var _isNotNil = _interopRequireDefault(require("../../isNotNil.js"));
var _isNotFunction = _interopRequireDefault(require("../../isNotFunction.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _toConsumableArray(r) { return _arrayWithoutHoles(r) || _iterableToArray(r) || _unsupportedIterableToArray(r) || _nonIterableSpread(); }
function _nonIterableSpread() { throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _iterableToArray(r) { if ("undefined" != typeof Symbol && null != r[Symbol.iterator] || null != r["@@iterator"]) return Array.from(r); }
function _arrayWithoutHoles(r) { if (Array.isArray(r)) return _arrayLikeToArray(r); }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
var copyArray = function copyArray(items, mapFn, thisArg) {
  var boundMapFn = (0, _isNotUndefined["default"])(thisArg) ? (0, _ramda.bind)(mapFn, thisArg) : mapFn;
  return (0, _isNotUndefined["default"])(mapFn) ? _toConsumableArray(items).map(boundMapFn) : _toConsumableArray(items);
};
var fromArray = function fromArray(items, mapFn, thisArg) {
  if (items == null) {
    throw new TypeError('Array.from requires an array-like object - not null or undefined');
  }
  if ((0, _isNotNil["default"])(mapFn) && (0, _isNotFunction["default"])(mapFn)) {
    throw new TypeError('Array.from: when provided, the second argument must be a function');
  }
  if ((0, _isIterable["default"])(items)) {
    return copyArray(items, mapFn, thisArg);
  }
  return [];
};
var _default = exports["default"] = fromArray;