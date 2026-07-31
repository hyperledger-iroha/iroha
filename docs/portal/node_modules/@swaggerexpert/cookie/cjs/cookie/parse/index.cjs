"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../../grammar.cjs"));
var _cookieString = _interopRequireDefault(require("./callbacks/cookie-string.cjs"));
var _cookiePair = _interopRequireDefault(require("./callbacks/cookie-pair.cjs"));
var _cookieName = _interopRequireDefault(require("./callbacks/cookie-name.cjs"));
var _cookieValue = _interopRequireDefault(require("./callbacks/cookie-value.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const grammar = new _grammar.default();
const parse = (cookieString, {
  strict = true
} = {}) => {
  const parser = new _apgLite.Parser();
  parser.ast = new _apgLite.Ast();
  if (strict) {
    parser.ast.callbacks['cookie-string'] = _cookieString.default;
    parser.ast.callbacks['cookie-pair'] = _cookiePair.default;
    parser.ast.callbacks['cookie-name'] = _cookieName.default;
    parser.ast.callbacks['cookie-value'] = _cookieValue.default;
  } else {
    parser.ast.callbacks['lenient-cookie-string'] = _cookieString.default;
    parser.ast.callbacks['lenient-cookie-pair'] = _cookiePair.default;
    parser.ast.callbacks['lenient-cookie-name'] = _cookieName.default;
    parser.ast.callbacks['lenient-cookie-value'] = _cookieValue.default;
  }
  const startRule = strict ? 'cookie-string' : 'lenient-cookie-string';
  const result = parser.parse(grammar, startRule, cookieString);
  return {
    result,
    ast: parser.ast
  };
};
var _default = exports.default = parse;