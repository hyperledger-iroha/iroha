"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class IbmmqServerBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'ibmmqServerBinding';
    this.classes.push('server-binding');
  }
  get groupId() {
    return this.get('groupId');
  }
  set groupId(groupId) {
    this.set('groupId', groupId);
  }
  get ccdtQueueManagerName() {
    return this.get('ccdtQueueManagerName');
  }
  set ccdtQueueManagerName(ccdtQueueManagerName) {
    this.set('ccdtQueueManagerName', ccdtQueueManagerName);
  }
  get cipherSpec() {
    return this.get('cipherSpec');
  }
  set cipherSpec(cipherSpec) {
    this.set('cipherSpec', cipherSpec);
  }
  get multiEndpointServer() {
    return this.get('multiEndpointServer');
  }
  set multiEndpointServer(multiEndpointServer) {
    this.set('multiEndpointServer', multiEndpointServer);
  }
  get heartBeatInterval() {
    return this.get('heartBeatInterval');
  }
  set heartBeatInterval(heartBeatInterval) {
    this.set('heartBeatInterval', heartBeatInterval);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = IbmmqServerBinding;