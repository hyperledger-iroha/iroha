import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class RedisOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'redisOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default RedisOperationBinding;