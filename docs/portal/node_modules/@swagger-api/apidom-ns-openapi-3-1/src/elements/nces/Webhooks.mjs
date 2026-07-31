import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Webhooks extends ObjectElement {
  static primaryClass = 'webhooks';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Webhooks.primaryClass);
  }
}
export default Webhooks;