"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _Expectations = _interopRequireDefault(require("./Expectations.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class Trace extends _apgLite.Trace {
  inferExpectations() {
    const lines = this.displayTrace().split('\n');
    const expectations = new Set();
    let collecting = false;
    let lastMatchedIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // capture the max match line (first one that ends in a single character match)
      if (!collecting && line.includes('M|')) {
        const textMatch = line.match(/]'(.*)'$/);
        if (textMatch && textMatch[1]) {
          lastMatchedIndex = i;
        }
      }

      // begin collecting after the deepest successful match
      if (i > lastMatchedIndex) {
        const terminalFailMatch = line.match(/N\|\[TLS\(([^)]+)\)]/);
        if (terminalFailMatch) {
          expectations.add(terminalFailMatch[1]);
        }
      }
    }
    return new _Expectations.default(...expectations);
  }
}
var _default = exports.default = Trace;