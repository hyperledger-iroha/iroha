"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOAuthFlowsVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _OAuthFlows = _interopRequireDefault(require("../../../../elements/OAuthFlows.cjs"));
/**
 * @public
 */
const BaseOAuthFlowsVisitor = exports.BaseOAuthFlowsVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlows.$visitor;
/**
 * @public
 */
class OAuthFlowsVisitor extends BaseOAuthFlowsVisitor {
  constructor(options) {
    super(options);
    this.element = new _OAuthFlows.default();
  }
}
var _default = exports.default = OAuthFlowsVisitor;