import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Amqp1OperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqp1OperationBinding';
    this.classes.push('operation-binding');
  }
}
export default Amqp1OperationBinding;