"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class GooglepubsubChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'googlepubsubChannelBinding';
    this.classes.push('channel-binding');
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
  get labels() {
    return this.get('labels');
  }
  set labels(labels) {
    this.set('labels', labels);
  }
  get messageRetentionDuration() {
    return this.get('messageRetentionDuration');
  }
  set messageRetentionDuration(messageRetentionDuration) {
    this.set('messageRetentionDuration', messageRetentionDuration);
  }
  get messageStoragePolicy() {
    return this.get('messageStoragePolicy');
  }
  set messageStoragePolicy(messageStoragePolicy) {
    this.set('messageStoragePolicy', messageStoragePolicy);
  }
  get schemaSettings() {
    return this.get('schemaSettings');
  }
  set schemaSettings(schemaSettings) {
    this.set('schemaSettings', schemaSettings);
  }
  get topic() {
    return this.get('topic');
  }
  set topic(topic) {
    this.set('topic', topic);
  }
}
var _default = exports.default = GooglepubsubChannelBinding;