import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ResponsesDefinitions extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'responsesDefinitions';
  }
}
export default ResponsesDefinitions;