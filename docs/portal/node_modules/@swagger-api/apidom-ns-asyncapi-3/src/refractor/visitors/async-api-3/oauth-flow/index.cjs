"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseOAuthFlowVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _OauthFlow = _interopRequireDefault(require("../../../../elements/OauthFlow.cjs"));
/**
 * @public
 */
const BaseOAuthFlowVisitor = exports.BaseOAuthFlowVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.OAuthFlow.$visitor;
/**
 * @public
 */
class OAuthFlowVisitor extends BaseOAuthFlowVisitor {
  constructor(options) {
    super(options);
    this.element = new _OauthFlow.default();
  }
}
var _default = exports.default = OAuthFlowVisitor;