"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isInteger = _interopRequireDefault(require("../../isInteger.js"));
var _NumberMAX_SAFE_INTEGER = _interopRequireDefault(require("./Number.MAX_SAFE_INTEGER.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isSafeIntegerPonyfill = (0, _ramda.both)(_isInteger["default"], function (value) {
  return Math.abs(value) <= _NumberMAX_SAFE_INTEGER["default"];
});
var _default = exports["default"] = isSafeIntegerPonyfill;