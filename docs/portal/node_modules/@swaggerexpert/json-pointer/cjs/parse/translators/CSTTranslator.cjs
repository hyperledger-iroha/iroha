"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../apg-lite.cjs");
var _cst = _interopRequireDefault(require("../callbacks/cst.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class CSTTranslator extends _apgLite.Ast {
  constructor() {
    super();
    this.callbacks['json-pointer'] = (0, _cst.default)('json-pointer');
    this.callbacks['reference-token'] = (0, _cst.default)('reference-token');
    this.callbacks['slash'] = (0, _cst.default)('text');
  }
  getTree() {
    const data = {
      stack: [],
      root: null
    };
    this.translate(data);
    delete data.stack;
    return data;
  }
}
var _default = exports.default = CSTTranslator;