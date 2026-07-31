import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ResponseHeaders extends ObjectElement {
  static primaryClass = 'response-headers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseHeaders.primaryClass);
  }
}
export default ResponseHeaders;