import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Reference extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'reference';
    this.classes.push('openapi-reference');
  }
  get $ref() {
    return this.get('$ref');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
  }
}
export default Reference;