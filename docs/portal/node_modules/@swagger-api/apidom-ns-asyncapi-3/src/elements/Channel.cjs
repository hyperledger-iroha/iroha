"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Channel extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'channel';
  }
  get address() {
    return this.get('address');
  }
  set address(address) {
    this.set('address', address);
  }
  get messages() {
    return this.get('messages');
  }
  set messages(messages) {
    this.set('messages', messages);
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get servers() {
    return this.get('servers');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
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
var _default = exports.default = Channel;