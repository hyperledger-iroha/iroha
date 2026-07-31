import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Mqtt5MessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mqtt5MessageBinding';
    this.classes.push('message-binding');
  }
}
export default Mqtt5MessageBinding;