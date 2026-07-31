import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class Responses extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'responses';
  }
  get default() {
    return this.get('default');
  }
  set default(defaultValue) {
    this.set('default', defaultValue);
  }
}
export default Responses;