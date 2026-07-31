"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports["default"] = void 0;
var _highlight = _interopRequireDefault(require("./highlight"));
var _core = require("refractor/core");
var SyntaxHighlighter = (0, _highlight["default"])(_core.refractor, {});
SyntaxHighlighter.registerLanguage = function (_, language) {
  return _core.refractor.register(language);
};
SyntaxHighlighter.alias = function (name, aliases) {
  return _core.refractor.alias(name, aliases);
};
var _default = exports["default"] = SyntaxHighlighter;