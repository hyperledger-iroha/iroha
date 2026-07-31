"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseSecuritySchemeVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _SecurityScheme = _interopRequireDefault(require("../../../../elements/SecurityScheme.cjs"));
/**
 * @public
 */
const BaseSecuritySchemeVisitor = exports.BaseSecuritySchemeVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.$visitor;
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