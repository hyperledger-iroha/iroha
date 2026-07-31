import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MercureServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mercureServerBinding';
    this.classes.push('server-binding');
  }
}
export default MercureServerBinding;