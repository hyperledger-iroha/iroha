import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Identifier extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'identifier';
  }
}
export default Identifier;