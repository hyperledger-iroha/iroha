"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _JSONPointerParseError = _interopRequireDefault(require("../../errors/JSONPointerParseError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cst = ruleName => {
  return (state, chars, phraseIndex, phraseLength, data) => {
    if (!(typeof data === 'object' && data !== null && !Array.isArray(data))) {
      throw new _JSONPointerParseError.default("parser's user data must be an object");
    }
    if (state === _apgLite.identifiers.SEM_PRE) {
      const node = {
        type: ruleName,
        text: _apgLite.utilities.charsToString(chars, phraseIndex, phraseLength),
        start: phraseIndex,
        length: phraseLength,
        children: []
      };
      if (data.stack.length > 0) {
        const parent = data.stack[data.stack.length - 1];
        parent.children.push(node);
      } else {
        data.root = node;
      }
      data.stack.push(node);
    }
    if (state === _apgLite.identifiers.SEM_POST) {
      data.stack.pop();
    }
  };
};
var _default = exports.default = cst;