"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class Info extends _apidomNsOpenapi.InfoElement {
  get license() {
    return this.get('license');
  }
  set license(licenseElement) {
    this.set('license', licenseElement);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
}
var _default = exports.default = Info;