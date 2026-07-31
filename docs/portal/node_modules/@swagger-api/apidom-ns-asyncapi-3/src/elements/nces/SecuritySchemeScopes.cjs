"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SecuritySchemeScopes extends _apidomCore.ArrayElement {
  static primaryClass = 'security-scheme-scopes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SecuritySchemeScopes.primaryClass);
  }
}
var _default = exports.default = SecuritySchemeScopes;