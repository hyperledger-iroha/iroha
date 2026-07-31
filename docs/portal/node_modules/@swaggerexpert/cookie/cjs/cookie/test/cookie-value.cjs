"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../../grammar.cjs"));
var _utils = require("../../utils.cjs");
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const parser = new _apgLite.Parser();
const grammar = new _grammar.default();
const testCookieValue = (cookieValue, {
  strict = true,
  quoted = null
} = {}) => {
  try {
    const startRule = strict ? 'cookie-value' : 'lenient-cookie-value';
    const result = parser.parse(grammar, startRule, cookieValue);
    if (typeof quoted === 'boolean') {
      return result.success && quoted === (0, _utils.isQuoted)(cookieValue);
    }
    return result.success;
  } catch {
    return false;
  }
};
var _default = exports.default = testCookieValue;