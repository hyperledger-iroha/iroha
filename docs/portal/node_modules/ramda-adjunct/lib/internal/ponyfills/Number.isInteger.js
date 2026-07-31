"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isFinite = _interopRequireDefault(require("../../isFinite.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isIntegerPonyfill = (0, _ramda.both)(_isFinite["default"], (0, _ramda.converge)(_ramda.equals, [Math.floor, _ramda.identity]));
var _default = exports["default"] = isIntegerPonyfill;