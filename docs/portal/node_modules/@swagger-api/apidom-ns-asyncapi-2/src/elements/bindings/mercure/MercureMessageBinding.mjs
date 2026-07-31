import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MercureMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mercureMessageBinding';
    this.classes.push('message-binding');
  }
}
export default MercureMessageBinding;