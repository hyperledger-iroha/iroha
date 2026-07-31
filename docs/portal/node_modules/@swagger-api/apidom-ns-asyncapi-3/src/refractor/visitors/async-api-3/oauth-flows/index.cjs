"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOAuthFlowsVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _OauthFlows = _interopRequireDefault(require("../../../../elements/OauthFlows.cjs"));
/**
 * @public
 */
const BaseOAuthFlowsVisitor = exports.BaseOAuthFlowsVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.OAuthFlows.$visitor;
/**
 * @public
 */
class OAuthFlowsVisitor extends BaseOAuthFlowsVisitor {
  constructor(options) {
    super(options);
    this.element = new _OauthFlows.default();
  }
}
var _default = exports.default = OAuthFlowsVisitor;