import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Reference extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'reference';
    this.classes.push('json-reference');
    this.classes.push('asyncapi-reference');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
  }
  get $ref() {
    return this.get('$ref');
  }
}
export default Reference;