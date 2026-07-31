"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class PathItem extends _apidomNsOpenapi.PathItemElement {
  get GET() {
    return this.get('get');
  }
  set GET(operation) {
    this.set('GET', operation);
  }
  get PUT() {
    return this.get('put');
  }
  set PUT(operation) {
    this.set('PUT', operation);
  }
  get POST() {
    return this.get('post');
  }
  set POST(operation) {
    this.set('POST', operation);
  }
  get DELETE() {
    return this.get('delete');
  }
  set DELETE(operation) {
    this.set('DELETE', operation);
  }
  get OPTIONS() {
    return this.get('options');
  }
  set OPTIONS(operation) {
    this.set('OPTIONS', operation);
  }
  get HEAD() {
    return this.get('head');
  }
  set HEAD(operation) {
    this.set('HEAD', operation);
  }
  get PATCH() {
    return this.get('patch');
  }
  set PATCH(operation) {
    this.set('PATCH', operation);
  }
  get TRACE() {
    return this.get('trace');
  }
  set TRACE(operation) {
    this.set('TRACE', operation);
  }
}
var _default = exports.default = PathItem;