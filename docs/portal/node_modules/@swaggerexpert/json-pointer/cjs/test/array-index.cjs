"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../grammar.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const grammar = new _grammar.default();
const parser = new _apgLite.Parser();
const testArrayIndex = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-index', referenceToken).success;
  } catch {
    return false;
  }
};
var _default = exports.default = testArrayIndex;