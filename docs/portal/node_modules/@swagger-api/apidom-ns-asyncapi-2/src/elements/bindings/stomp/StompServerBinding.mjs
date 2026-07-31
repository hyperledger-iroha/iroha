import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StompServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'stompServerBinding';
    this.classes.push('server-binding');
  }
}
export default StompServerBinding;