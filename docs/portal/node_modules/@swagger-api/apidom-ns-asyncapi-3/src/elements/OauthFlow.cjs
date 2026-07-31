"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class OAuthFlow extends _apidomNsAsyncapi.OAuthFlowElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'oAuthFlow';
  }
  get scopes() {
    throw new _apidomError.UnsupportedOperationError('scopes keyword from Core vocabulary has been renamed to availableScopes.');
  }
  set scopes(scopes) {
    throw new _apidomError.UnsupportedOperationError('scopes keyword from Core vocabulary has been renamed to availableScopes.');
  }
  get availableScopes() {
    return this.get('availableScopes');
  }
  set availableScopes(availableScopes) {
    this.set('availableScopes', availableScopes);
  }
}
var _default = exports.default = OAuthFlow;