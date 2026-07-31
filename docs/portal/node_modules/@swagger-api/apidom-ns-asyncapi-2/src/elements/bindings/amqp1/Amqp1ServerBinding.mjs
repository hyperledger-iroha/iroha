import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Amqp1ServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqp1ServerBinding';
    this.classes.push('server-binding');
  }
}
export default Amqp1ServerBinding;