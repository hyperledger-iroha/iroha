"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationTrait extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationTrait';
  }
  get operationId() {
    return this.get('operationId');
  }
  set operationId(operationId) {
    this.set('operationId', operationId);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get security() {
    return this.get('security');
  }
  set security(security) {
    this.set('security', security);
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
  get bindings() {
    return this.get('bindings');
  }
  set bindings(bindings) {
    this.set('bindings', bindings);
  }
}
var _default = exports.default = OperationTrait;