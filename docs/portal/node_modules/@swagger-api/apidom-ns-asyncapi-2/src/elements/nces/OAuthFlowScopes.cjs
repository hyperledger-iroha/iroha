"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OAuthFlowScopes extends _apidomCore.ObjectElement {
  static primaryClass = 'oauth-flow-scopes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OAuthFlowScopes.primaryClass);
  }
}
var _default = exports.default = OAuthFlowScopes;