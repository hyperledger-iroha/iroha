"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/**
 * @public
 */
class SecurityScheme extends _apidomNsAsyncapi.SecuritySchemeElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'securityScheme';
  }
  get scopes() {
    return this.get('scopes');
  }
  set scopes(scopes) {
    this.set('scopes', scopes);
  }
}
var _default = exports.default = SecurityScheme;