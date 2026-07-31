import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Example extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'example';
  }
}
export default Example;