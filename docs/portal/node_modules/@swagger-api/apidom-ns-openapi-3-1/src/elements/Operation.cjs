"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class Operation extends _apidomNsOpenapi.OperationElement {
  get requestBody() {
    return this.get('requestBody');
  }
  set requestBody(requestBody) {
    this.set('requestBody', requestBody);
  }
}
var _default = exports.default = Operation;