import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationReplyAddress extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationReplyAddress';
  }
  get description() {
    return this.get('description');
  }
  set description(value) {
    this.set('description', value);
  }
  get location() {
    return this.get('location');
  }
  set location(value) {
    this.set('location', value);
  }
}
export default OperationReplyAddress;