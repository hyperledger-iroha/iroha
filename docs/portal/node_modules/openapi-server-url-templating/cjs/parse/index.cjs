"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../apg-lite.cjs");
var _serverUrlTemplating = _interopRequireDefault(require("../server-url-templating.cjs"));
var _serverUrlTemplate = _interopRequireDefault(require("./callbacks/server-url-template.cjs"));
var _serverVariable = _interopRequireDefault(require("./callbacks/server-variable.cjs"));
var _serverVariableName = _interopRequireDefault(require("./callbacks/server-variable-name.cjs"));
var _literals = _interopRequireDefault(require("./callbacks/literals.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const grammar = new _serverUrlTemplating.default();
const parse = serverURLTemplate => {
  const parser = new _apgLite.Parser();
  parser.ast = new _apgLite.Ast();
  parser.ast.callbacks['server-url-template'] = _serverUrlTemplate.default;
  parser.ast.callbacks['server-variable'] = _serverVariable.default;
  parser.ast.callbacks['server-variable-name'] = _serverVariableName.default;
  parser.ast.callbacks['literals'] = _literals.default;
  const result = parser.parse(grammar, 'server-url-template', serverURLTemplate);
  return {
    result,
    ast: parser.ast
  };
};
var _default = exports.default = parse;