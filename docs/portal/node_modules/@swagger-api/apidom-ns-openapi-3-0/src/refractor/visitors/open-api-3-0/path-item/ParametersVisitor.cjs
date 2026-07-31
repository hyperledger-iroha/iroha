"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _PathItemParameters = _interopRequireDefault(require("../../../../elements/nces/PathItemParameters.cjs"));
var _ParametersVisitor = _interopRequireDefault(require("../ParametersVisitor.cjs"));
/**
 * @public
 */
class ParametersVisitor extends _ParametersVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _PathItemParameters.default();
  }
}
var _default = exports.default = ParametersVisitor;