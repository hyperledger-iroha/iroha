"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _OperationSchemes = _interopRequireDefault(require("../../../../elements/nces/OperationSchemes.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */
class SchemesVisitor extends _FallbackVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _OperationSchemes.default();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat((0, _apidomCore.cloneDeep)(arrayElement));
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SchemesVisitor;