import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Definitions extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'definitions';
  }
}
export default Definitions;