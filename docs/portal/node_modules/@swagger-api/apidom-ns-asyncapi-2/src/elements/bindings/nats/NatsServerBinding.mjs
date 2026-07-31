import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class NatsServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'natsServerBinding';
    this.classes.push('server-binding');
  }
}
export default NatsServerBinding;