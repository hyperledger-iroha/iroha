"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class KafkaOperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'kafkaOperationBinding';
    this.classes.push('operation-binding');
  }
  get groupId() {
    return this.get('groupId');
  }
  set groupId(groupId) {
    this.set('groupId', groupId);
  }
  get clientId() {
    return this.get('clientId');
  }
  set clientId(clientId) {
    this.set('clientId', clientId);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = KafkaOperationBinding;