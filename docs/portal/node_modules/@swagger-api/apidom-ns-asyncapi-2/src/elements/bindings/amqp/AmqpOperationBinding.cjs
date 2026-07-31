"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class AmqpOperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqpOperationBinding';
    this.classes.push('operation-binding');
  }
  get expiration() {
    return this.get('expiration');
  }
  set expiration(expiration) {
    this.set('expiration', expiration);
  }
  get userId() {
    return this.get('userId');
  }
  set userId(userId) {
    this.set('userId', userId);
  }
  get cc() {
    return this.get('cc');
  }
  set cc(cc) {
    this.set('cc', cc);
  }
  get priority() {
    return this.get('priority');
  }
  set priority(priority) {
    this.set('priority', priority);
  }
  get deliveryMode() {
    return this.get('deliveryMode');
  }
  set deliveryMode(deliveryMode) {
    this.set('deliveryMode', deliveryMode);
  }
  get mandatory() {
    return this.get('mandatory');
  }
  set mandatory(mandatory) {
    this.set('mandatory', mandatory);
  }
  get bcc() {
    return this.get('bcc');
  }
  set bcc(bcc) {
    this.set('bcc', bcc);
  }
  get replyTo() {
    return this.get('replyTo');
  }
  set replyTo(replyTo) {
    this.set('replyTo', replyTo);
  }
  get timestamp() {
    return this.get('timestamp');
  }
  set timestamp(timestamp) {
    this.set('timestamp', timestamp);
  }
  get ack() {
    return this.get('ack');
  }
  set ack(ack) {
    this.set('ack', ack);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = AmqpOperationBinding;