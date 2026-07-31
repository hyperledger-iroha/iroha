"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var trimStart = (0, _ramda.replace)(/[\s\uFEFF\xA0]+$/, '');
var _default = exports["default"] = trimStart;