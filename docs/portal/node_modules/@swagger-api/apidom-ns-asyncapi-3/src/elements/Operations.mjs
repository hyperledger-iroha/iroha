import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Operations extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operations';
  }
}
export default Operations;