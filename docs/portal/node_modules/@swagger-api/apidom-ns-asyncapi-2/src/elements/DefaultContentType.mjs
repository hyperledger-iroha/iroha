import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class DefaultContentType extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'defaultContentType';
  }
}
export default DefaultContentType;