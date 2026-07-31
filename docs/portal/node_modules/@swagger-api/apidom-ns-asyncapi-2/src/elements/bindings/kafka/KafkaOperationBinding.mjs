import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class KafkaOperationBinding extends ObjectElement {
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
export default KafkaOperationBinding;