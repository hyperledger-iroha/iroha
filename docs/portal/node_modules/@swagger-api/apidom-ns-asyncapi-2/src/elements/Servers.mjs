import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Servers extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'servers';
  }
}
export default Servers;