import { MessageTraitElement } from '@swagger-api/apidom-ns-asyncapi-2';
import { UnsupportedOperationError } from '@swagger-api/apidom-error';
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class MessageTrait extends MessageTraitElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'messageTrait';
  }
  get messageId() {
    throw new UnsupportedOperationError('messageId keyword from Core vocabulary has been removed');
  }
  set messageId(messageId) {
    throw new UnsupportedOperationError('messageId keyword from Core vocabulary has been removed');
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default MessageTrait;