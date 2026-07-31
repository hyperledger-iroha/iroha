"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ExamplesVisitor = _interopRequireDefault(require("../ExamplesVisitor.cjs"));
var _ParameterExamples = _interopRequireDefault(require("../../../../elements/nces/ParameterExamples.cjs"));
/**
 * @public
 */
class ExamplesVisitor extends _ExamplesVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _ParameterExamples.default();
  }
}
var _default = exports.default = ExamplesVisitor;