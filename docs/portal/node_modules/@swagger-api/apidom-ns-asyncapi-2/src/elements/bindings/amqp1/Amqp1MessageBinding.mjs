import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Amqp1MessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqp1MessageBinding';
    this.classes.push('message-binding');
  }
}
export default Amqp1MessageBinding;