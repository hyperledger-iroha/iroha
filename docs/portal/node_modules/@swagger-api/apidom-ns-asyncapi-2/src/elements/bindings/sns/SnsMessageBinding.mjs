import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SnsMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'snsMessageBinding';
    this.classes.push('message-binding');
  }
}
export default SnsMessageBinding;