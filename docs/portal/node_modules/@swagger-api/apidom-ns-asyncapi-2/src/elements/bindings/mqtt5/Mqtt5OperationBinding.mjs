import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Mqtt5OperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mqtt5OperationBinding';
    this.classes.push('operation-binding');
  }
}
export default Mqtt5OperationBinding;