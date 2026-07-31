"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class HttpOperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'httpOperationBinding';
    this.classes.push('operation-binding');
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get method() {
    return this.get('method');
  }
  set method(method) {
    this.set('method', method);
  }
  get query() {
    return this.get('query');
  }
  set query(query) {
    this.set('query', query);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = HttpOperationBinding;