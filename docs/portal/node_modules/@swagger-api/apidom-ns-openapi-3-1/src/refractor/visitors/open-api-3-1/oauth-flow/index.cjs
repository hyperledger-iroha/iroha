"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOAuthFlowVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _OAuthFlow = _interopRequireDefault(require("../../../../elements/OAuthFlow.cjs"));
/**
 * @public
 */
const BaseOAuthFlowVisitor = exports.BaseOAuthFlowVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlow.$visitor;
/**
 * @public
 */
class OAuthFlowVisitor extends BaseOAuthFlowVisitor {
  constructor(options) {
    super(options);
    this.element = new _OAuthFlow.default();
  }
}
var _default = exports.default = OAuthFlowVisitor;