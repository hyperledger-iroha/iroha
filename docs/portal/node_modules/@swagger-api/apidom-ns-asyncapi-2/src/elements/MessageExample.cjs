"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MessageExample extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'messageExample';
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get payload() {
    return this.get('payload');
  }
  set payload(payload) {
    this.set('payload', payload);
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
}
var _default = exports.default = MessageExample;