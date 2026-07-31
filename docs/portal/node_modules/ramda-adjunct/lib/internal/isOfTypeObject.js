"use strict";

exports.__esModule = true;
exports["default"] = void 0;
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
var isOfTypeObject = function isOfTypeObject(val) {
  return _typeof(val) === 'object';
};
var _default = exports["default"] = isOfTypeObject;