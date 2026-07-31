"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseServerVariableVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _ServerVariable = _interopRequireDefault(require("../../../../elements/ServerVariable.cjs"));
/**
 * @public
 */
const BaseServerVariableVisitor = exports.BaseServerVariableVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.ServerVariable.$visitor;
/**
 * @public
 */
class ServerVariableVisitor extends BaseServerVariableVisitor {
  constructor(options) {
    super(options);
    this.element = new _ServerVariable.default();
  }
}
var _default = exports.default = ServerVariableVisitor;