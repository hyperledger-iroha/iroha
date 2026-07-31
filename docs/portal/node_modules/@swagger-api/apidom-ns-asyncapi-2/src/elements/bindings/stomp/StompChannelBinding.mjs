import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StompChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'stompChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default StompChannelBinding;