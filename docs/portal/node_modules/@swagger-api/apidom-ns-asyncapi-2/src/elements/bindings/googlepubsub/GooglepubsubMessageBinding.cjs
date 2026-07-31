"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class GooglepubsubMessageBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'googlepubsubMessageBinding';
    this.classes.push('message-binding');
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
  get attributesProp() {
    return this.get('attributes');
  }
  set attributesProp(attributes) {
    this.set('attributes', attributes);
  }
  get orderingKey() {
    return this.get('orderingKey');
  }
  set orderingKey(orderingKey) {
    this.set('orderingKey', orderingKey);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
var _default = exports.default = GooglepubsubMessageBinding;