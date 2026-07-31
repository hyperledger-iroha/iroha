"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOperationTraitVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _OperationTrait = _interopRequireDefault(require("../../../../elements/OperationTrait.cjs"));
const BaseOperationTraitVisitor = exports.BaseOperationTraitVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.OperationTrait.$visitor;
/**
 * @public
 */
class OperationTraitVisitor extends BaseOperationTraitVisitor {
  constructor(options) {
    super(options);
    this.element = new _OperationTrait.default();
  }
}
var _default = exports.default = OperationTraitVisitor;