"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _RequestBodyContent = _interopRequireDefault(require("../../../../elements/nces/RequestBodyContent.cjs"));
var _ContentVisitor = _interopRequireDefault(require("../ContentVisitor.cjs"));
/**
 * @public
 */
class ContentVisitor extends _ContentVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _RequestBodyContent.default();
  }
}
var _default = exports.default = ContentVisitor;