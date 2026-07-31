import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SqsMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'sqsMessageBinding';
    this.classes.push('message-binding');
  }
}
export default SqsMessageBinding;