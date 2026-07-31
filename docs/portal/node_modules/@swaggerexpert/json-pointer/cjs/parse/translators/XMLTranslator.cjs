"use strict";

exports.__esModule = true;
exports.default = void 0;
var _CSTTranslator = _interopRequireDefault(require("./CSTTranslator.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class XMLTranslator extends _CSTTranslator.default {
  getTree() {
    return this.toXml();
  }
}
var _default = exports.default = XMLTranslator;