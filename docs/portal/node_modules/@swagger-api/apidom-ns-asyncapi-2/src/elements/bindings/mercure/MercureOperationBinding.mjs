import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MercureOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mercureOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default MercureOperationBinding;