"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOperationBindingsVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _OperationBindings = _interopRequireDefault(require("../../../../elements/OperationBindings.cjs"));
const BaseOperationBindingsVisitor = exports.BaseOperationBindingsVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.OperationBindings.$visitor;
/**
 * @public
 */
class OperationBindingsVisitor extends BaseOperationBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new _OperationBindings.default();
  }
}
var _default = exports.default = OperationBindingsVisitor;