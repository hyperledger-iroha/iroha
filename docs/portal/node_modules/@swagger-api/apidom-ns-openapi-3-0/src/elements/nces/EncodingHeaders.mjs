import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class EncodingHeaders extends ObjectElement {
  static primaryClass = 'encoding-headers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(EncodingHeaders.primaryClass);
  }
}
export default EncodingHeaders;