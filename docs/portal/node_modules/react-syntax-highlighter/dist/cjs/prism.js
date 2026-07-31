"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports["default"] = void 0;
var _highlight = _interopRequireDefault(require("./highlight"));
var _prism = _interopRequireDefault(require("./styles/prism/prism"));
var _all = require("refractor/all");
var _supportedLanguages = _interopRequireDefault(require("./languages/prism/supported-languages"));
var highlighter = (0, _highlight["default"])(_all.refractor, _prism["default"]);
highlighter.supportedLanguages = _supportedLanguages["default"];
var _default = exports["default"] = highlighter;