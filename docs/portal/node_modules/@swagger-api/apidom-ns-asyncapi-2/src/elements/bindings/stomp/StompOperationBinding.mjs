import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StompOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'stompOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default StompOperationBinding;