import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class PulsarMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'pulsarMessageBinding';
    this.classes.push('message-binding');
  }
}
export default PulsarMessageBinding;