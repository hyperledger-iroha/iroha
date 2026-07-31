"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class License extends _apidomNsOpenapi.LicenseElement {
  get identifier() {
    return this.get('identifier');
  }
  set identifier(name) {
    this.set('identifier', name);
  }
}
var _default = exports.default = License;