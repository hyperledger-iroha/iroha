"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _allP = _interopRequireDefault(require("../../allP.js"));
var _resolveP = _interopRequireDefault(require("../../resolveP.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
function _toConsumableArray(r) { return _arrayWithoutHoles(r) || _iterableToArray(r) || _unsupportedIterableToArray(r) || _nonIterableSpread(); }
function _nonIterableSpread() { throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _iterableToArray(r) { if ("undefined" != typeof Symbol && null != r[Symbol.iterator] || null != r["@@iterator"]) return Array.from(r); }
function _arrayWithoutHoles(r) { if (Array.isArray(r)) return _arrayLikeToArray(r); }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
var onFulfill = function onFulfill(value) {
  return {
    status: 'fulfilled',
    value: value
  };
};
var onReject = function onReject(reason) {
  return {
    status: 'rejected',
    reason: reason
  };
};
var allSettledPonyfill = function allSettledPonyfill(iterable) {
  var array = (0, _ramda.map)(function (p) {
    return (0, _resolveP["default"])(p).then(onFulfill)["catch"](onReject);
  }, _toConsumableArray(iterable));
  return (0, _allP["default"])(array);
};
var _default = exports["default"] = allSettledPonyfill;