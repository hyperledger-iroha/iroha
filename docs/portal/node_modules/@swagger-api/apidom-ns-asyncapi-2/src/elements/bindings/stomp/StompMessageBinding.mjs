import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StompMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'stompMessageBinding';
    this.classes.push('message-binding');
  }
}
export default StompMessageBinding;