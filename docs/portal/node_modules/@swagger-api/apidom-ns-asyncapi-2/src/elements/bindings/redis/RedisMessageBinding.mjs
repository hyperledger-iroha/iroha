import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class RedisMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'redisMessageBinding';
    this.classes.push('message-binding');
  }
}
export default RedisMessageBinding;