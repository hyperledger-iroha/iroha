import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Paths extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'paths';
  }
}
export default Paths;