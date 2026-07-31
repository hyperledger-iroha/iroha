import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ResponseContent extends ObjectElement {
  static primaryClass = 'response-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseContent.primaryClass);
    this.classes.push('content');
  }
}
export default ResponseContent;