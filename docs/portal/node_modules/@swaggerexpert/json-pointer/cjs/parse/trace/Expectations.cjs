"use strict";

exports.__esModule = true;
exports.default = void 0;
class Expectations extends Array {
  toString() {
    return this.map(c => `"${String(c)}"`).join(', ');
  }
}
var _default = exports.default = Expectations;