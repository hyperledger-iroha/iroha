import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Amqp1ChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqp1ChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default Amqp1ChannelBinding;