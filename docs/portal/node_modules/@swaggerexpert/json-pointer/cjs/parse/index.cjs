"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../grammar.cjs"));
var _JSONPointerParseError = _interopRequireDefault(require("../errors/JSONPointerParseError.cjs"));
var _ASTTranslator = _interopRequireDefault(require("./translators/ASTTranslator.cjs"));
var _Trace = _interopRequireDefault(require("./trace/Trace.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const grammar = new _grammar.default();
const parse = (jsonPointer, {
  translator = new _ASTTranslator.default(),
  stats = false,
  trace = false
} = {}) => {
  if (typeof jsonPointer !== 'string') {
    throw new TypeError('JSON Pointer must be a string');
  }
  try {
    const parser = new _apgLite.Parser();
    if (translator) parser.ast = translator;
    if (stats) parser.stats = new _apgLite.Stats();
    if (trace) parser.trace = new _Trace.default();
    const result = parser.parse(grammar, 'json-pointer', jsonPointer);
    return {
      result,
      tree: result.success && translator ? parser.ast.getTree() : undefined,
      stats: parser.stats,
      trace: parser.trace
    };
  } catch (error) {
    throw new _JSONPointerParseError.default('Unexpected error during JSON Pointer parsing', {
      cause: error,
      jsonPointer
    });
  }
};
var _default = exports.default = parse;