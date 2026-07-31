import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class GooglepubsubServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'googlepubsubServerBinding';
    this.classes.push('server-binding');
  }
}
export default GooglepubsubServerBinding;