"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _treeSitter = _interopRequireDefault(require("tree-sitter"));
var _treeSitterJson = _interopRequireDefault(require("tree-sitter-json"));
const parser = new _treeSitter.default();
parser.setLanguage(_treeSitterJson.default);

/**
 * Lexical Analysis of source string using TreeSitter.
 * This is Node.js version of TreeSitters Lexical Analysis.
 * @public
 */
const analyze = async source => {
  return parser.parse(source);
};
var _default = exports.default = analyze;