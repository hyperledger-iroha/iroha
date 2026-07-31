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
  get action() {
    return this.get('action');
  }
  set action(value) {
    this.set('action', value);
  }
  get channel() {
    return this.get('channel');
  }
  set channel(channel) {
    this.set('channel', channel);
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
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
  get messages() {
    return this.get('messages');
  }
  set messages(message) {
    this.set('messages', message);
  }
  get traits() {
    return this.get('traits');
  }
  set traits(OperationTrait) {
    this.set('traits', OperationTrait);
  }
}
var _default = exports.default = Operation;