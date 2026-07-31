"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var _isObj = _interopRequireDefault(require("../isObj.js"));
var _isSymbol = _interopRequireDefault(require("../isSymbol.js"));
var _neither = _interopRequireDefault(require("../neither.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var isCoercible = (0, _neither["default"])(_isSymbol["default"], (0, _ramda.both)(_isObj["default"], (0, _neither["default"])((0, _ramda.hasIn)('toString'), (0, _ramda.hasIn)('valueOf'))));
var _default = exports["default"] = isCoercible;