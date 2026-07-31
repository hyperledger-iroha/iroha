import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class IbmmqOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'ibmmqOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default IbmmqOperationBinding;