import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class RedisChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'redisChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default RedisChannelBinding;