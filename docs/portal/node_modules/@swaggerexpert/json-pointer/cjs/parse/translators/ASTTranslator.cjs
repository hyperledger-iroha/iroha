"use strict";

exports.__esModule = true;
exports.default = void 0;
var _CSTTranslator = _interopRequireDefault(require("./CSTTranslator.cjs"));
var _unescape = _interopRequireDefault(require("../../unescape.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class ASTTranslator extends _CSTTranslator.default {
  getTree() {
    const {
      root
    } = super.getTree();
    return root.children.filter(({
      type
    }) => type === 'reference-token').map(({
      text
    }) => (0, _unescape.default)(text));
  }
}
var _default = exports.default = ASTTranslator;