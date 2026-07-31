"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Operation extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operation';
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(description) {
    this.set('summary', description);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  get operationId() {
    return this.get('operationId');
  }
  set operationId(operationId) {
    this.set('operationId', operationId);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
  get requestBody() {
    return this.get('requestBody');
  }
  set requestBody(requestBody) {
    this.set('requestBody', requestBody);
  }
  get responses() {
    return this.get('responses');
  }
  set responses(responses) {
    this.set('responses', responses);
  }
  get callbacks() {
    return this.get('callbacks');
  }
  set callbacks(callbacks) {
    this.set('callbacks', callbacks);
  }
  get deprecated() {
    if (this.hasKey('deprecated')) {
      return this.get('deprecated');
    }
    return new _apidomCore.BooleanElement(false);
  }
  set deprecated(deprecated) {
    this.set('deprecated', deprecated);
  }
  get security() {
    return this.get('security');
  }
  set security(security) {
    this.set('security', security);
  }
  get servers() {
    return this.get('severs');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
}
var _default = exports.default = Operation;