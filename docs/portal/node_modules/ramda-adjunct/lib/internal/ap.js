"use strict";

function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFunction = _interopRequireDefault(require("../isFunction.js"));
var fl = _interopRequireWildcard(require("../fantasy-land/mapping.js"));
function _getRequireWildcardCache(e) { if ("function" != typeof WeakMap) return null; var r = new WeakMap(), t = new WeakMap(); return (_getRequireWildcardCache = function _getRequireWildcardCache(e) { return e ? t : r; })(e); }
function _interopRequireWildcard(e, r) { if (!r && e && e.__esModule) return e; if (null === e || "object" != _typeof(e) && "function" != typeof e) return { "default": e }; var t = _getRequireWildcardCache(r); if (t && t.has(e)) return t.get(e); var n = { __proto__: null }, a = Object.defineProperty && Object.getOwnPropertyDescriptor; for (var u in e) if ("default" !== u && {}.hasOwnProperty.call(e, u)) { var i = a ? Object.getOwnPropertyDescriptor(e, u) : null; i && (i.get || i.set) ? Object.defineProperty(n, u, i) : n[u] = e[u]; } return n["default"] = e, t && t.set(e, n), n; }
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isFunctor = (0, _ramda.either)((0, _ramda.pathSatisfies)(_isFunction["default"], ['map']), (0, _ramda.pathSatisfies)(_isFunction["default"], [fl.map]));
var isApply = (0, _ramda.both)(isFunctor, (0, _ramda.either)((0, _ramda.pathSatisfies)(_isFunction["default"], ['ap']), (0, _ramda.pathSatisfies)(_isFunction["default"], [fl.ap])));
var ap = (0, _ramda.curryN)(2, function (applyF, applyX) {
  // return original ramda `ap` if not Apply spec
  if (!isApply(applyF) || !isApply(applyX)) {
    return (0, _ramda.ap)(applyF, applyX);
  }
  try {
    // new version of `ap` starting from ramda version > 0.23.0
    return applyF.ap(applyX);
  } catch (e) {
    // old version of `ap` till ramda version <= 0.23.0
    return applyX.ap(applyF);
  }
});
var _default = exports["default"] = ap;