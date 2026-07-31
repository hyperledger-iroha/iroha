import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class AmqpMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqpMessageBinding';
    this.classes.push('message-binding');
  }
  get contentEncoding() {
    return this.get('contentEncoding');
  }
  set contentEncoding(contentEncoding) {
    this.set('contentEncoding', contentEncoding);
  }
  get messageType() {
    return this.get('messageType');
  }
  set messageType(messageType) {
    this.set('messageType', messageType);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
export default AmqpMessageBinding;