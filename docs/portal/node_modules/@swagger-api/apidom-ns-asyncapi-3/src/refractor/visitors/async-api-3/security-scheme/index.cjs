"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseSecuritySchemeVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _SecurityScheme = _interopRequireDefault(require("../../../../elements/SecurityScheme.cjs"));
const BaseSecuritySchemeVisitor = exports.BaseSecuritySchemeVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.SecurityScheme.$visitor;
/**
 * @public
 */
class SecuritySchemeVisitor extends BaseSecuritySchemeVisitor {
  constructor(options) {
    super(options);
    this.element = new _SecurityScheme.default();
  }
}
var _default = exports.default = SecuritySchemeVisitor;