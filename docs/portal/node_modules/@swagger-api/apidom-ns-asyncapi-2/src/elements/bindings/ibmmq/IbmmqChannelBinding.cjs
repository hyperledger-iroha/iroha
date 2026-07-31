"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class IbmmqChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'ibmmqChannelBinding';
    this.classes.push('channel-binding');
  }
  get destinationType() {
    return this.get('destinationType');
  }
  set destinationType(destinationType) {
    this.set('destinationType', destinationType);
  }
  get queue() {
    return this.get('queue');
  }
  set queue(queue) {
    this.set('queue', queue);
  }
  get topic() {
    return this.get('topic');
  }
  set topic(topic) {
    this.set('topic', topic);
  }
  get maxMsgLength() {
    return this.get('maxMsgLength');
  }
  set maxMsgLength(maxMsgLength) {
    this.set('maxMsgLength', maxMsgLength);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = IbmmqChannelBinding;