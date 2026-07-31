"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class PulsarChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'pulsarChannelBinding';
    this.classes.push('channel-binding');
  }
  get namespace() {
    return this.get('namespace');
  }
  set namespace(namespace) {
    this.set('namespace', namespace);
  }
  get persistence() {
    return this.get('persistence');
  }
  set persistence(persistence) {
    this.set('persistence', persistence);
  }
  get compaction() {
    return this.get('compaction');
  }
  set compaction(compaction) {
    this.set('compaction', compaction);
  }
  get ['geo-replication']() {
    return this.get('compaction');
  }
  set ['geo-replication'](geoReplication) {
    this.set('geo-replication', geoReplication);
  }
  get retention() {
    return this.get('retention');
  }
  set retention(retention) {
    this.set('retention', retention);
  }
  get ttl() {
    return this.get('ttl');
  }
  set ttl(ttl) {
    this.set('ttl', ttl);
  }
  get deduplication() {
    return this.get('deduplication');
  }
  set deduplication(deduplication) {
    this.set('deduplication', deduplication);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = PulsarChannelBinding;