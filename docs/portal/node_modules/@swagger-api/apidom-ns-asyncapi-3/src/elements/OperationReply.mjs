import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class OperationReply extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationReply';
  }
  get address() {
    return this.get('address');
  }
  set address(address) {
    this.set('address', address);
  }
  get channel() {
    return this.get('channel');
  }
  set channel(channel) {
    this.set('channel', channel);
  }
  get messages() {
    return this.get('messages');
  }
  set messages(messages) {
    this.set('messages', messages);
  }
}
export default OperationReply;