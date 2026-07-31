import { MessageElement } from '@swagger-api/apidom-ns-asyncapi-2';
import { UnsupportedOperationError } from '@swagger-api/apidom-error';
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class Message extends MessageElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'message';
  }
  get messageId() {
    throw new UnsupportedOperationError('messageId has been removed');
  }
  set messageId(messageId) {
    throw new UnsupportedOperationError('messageId has been removed');
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get payload() {
    return this.get('payload');
  }
  set payload(payload) {
    this.set('payload', payload);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default Message;