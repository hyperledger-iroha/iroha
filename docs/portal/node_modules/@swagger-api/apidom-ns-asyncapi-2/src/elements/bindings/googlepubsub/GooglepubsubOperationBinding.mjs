import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class GooglepubsubOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'googlepubsubOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default GooglepubsubOperationBinding;