import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Callback extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'callback';
  }
}
export default Callback;