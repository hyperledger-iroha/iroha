import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class PulsarOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'pulsarOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default PulsarOperationBinding;