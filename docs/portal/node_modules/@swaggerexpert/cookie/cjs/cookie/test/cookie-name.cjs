"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../../grammar.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const parser = new _apgLite.Parser();
const grammar = new _grammar.default();
const testCookieName = (cookieName, {
  strict = true
} = {}) => {
  try {
    const startRule = strict ? 'cookie-name' : 'lenient-cookie-name';
    return parser.parse(grammar, startRule, cookieName).success;
  } catch {
    return false;
  }
};
var _default = exports.default = testCookieName;