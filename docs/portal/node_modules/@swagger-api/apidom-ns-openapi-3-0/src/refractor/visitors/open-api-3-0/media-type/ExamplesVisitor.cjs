"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ExamplesVisitor = _interopRequireDefault(require("../ExamplesVisitor.cjs"));
var _MediaTypeExamples = _interopRequireDefault(require("../../../../elements/nces/MediaTypeExamples.cjs"));
/**
 * @public
 */
class ExamplesVisitor extends _ExamplesVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _MediaTypeExamples.default();
  }
}
var _default = exports.default = ExamplesVisitor;