import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SnsServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'snsServerBinding';
    this.classes.push('server-binding');
  }
}
export default SnsServerBinding;