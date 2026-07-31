import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class AmqpServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqpServerBinding';
    this.classes.push('server-binding');
  }
}
export default AmqpServerBinding;