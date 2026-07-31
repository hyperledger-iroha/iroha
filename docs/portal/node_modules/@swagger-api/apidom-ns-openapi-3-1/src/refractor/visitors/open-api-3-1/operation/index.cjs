"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOperationVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Operation = _interopRequireDefault(require("../../../../elements/Operation.cjs"));
/**
 * @public
 */
const BaseOperationVisitor = exports.BaseOperationVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.$visitor;
/**
 * @public
 */
class OperationVisitor extends BaseOperationVisitor {
  constructor(options) {
    super(options);
    this.element = new _Operation.default();
  }
}
var _default = exports.default = OperationVisitor;