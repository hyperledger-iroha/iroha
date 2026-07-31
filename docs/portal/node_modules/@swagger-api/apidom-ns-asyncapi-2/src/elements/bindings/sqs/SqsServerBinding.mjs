import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SqsServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'sqsServerBinding';
    this.classes.push('server-binding');
  }
}
export default SqsServerBinding;