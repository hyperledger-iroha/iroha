"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isNumber = _interopRequireDefault(require("../../isNumber.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
// eslint-disable-next-line no-restricted-globals
var isFinitePonyfill = (0, _ramda.both)(_isNumber["default"], isFinite);
var _default = exports["default"] = isFinitePonyfill;