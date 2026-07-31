import { ObjectElement, BooleanElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class RequestBody extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'requestBody';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get contentProp() {
    return this.get('content');
  }
  set contentProp(content) {
    this.set('content', content);
  }
  get required() {
    if (this.hasKey('required')) {
      return this.get('required');
    }
    return new BooleanElement(false);
  }
  set required(required) {
    this.set('required', required);
  }
}
export default RequestBody;