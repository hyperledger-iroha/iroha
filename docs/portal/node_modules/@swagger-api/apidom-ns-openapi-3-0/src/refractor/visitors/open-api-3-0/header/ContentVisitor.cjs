"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ContentVisitor = _interopRequireDefault(require("../ContentVisitor.cjs"));
var _HeaderContent = _interopRequireDefault(require("../../../../elements/nces/HeaderContent.cjs"));
/**
 * @public
 */
class ContentVisitor extends _ContentVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _HeaderContent.default();
  }
}
var _default = exports.default = ContentVisitor;