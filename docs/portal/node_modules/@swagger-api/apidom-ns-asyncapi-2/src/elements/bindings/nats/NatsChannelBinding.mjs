import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class NatsChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'natsChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default NatsChannelBinding;