import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ResponseLinks extends ObjectElement {
  static primaryClass = 'response-links';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseLinks.primaryClass);
  }
}
export default ResponseLinks;