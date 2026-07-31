import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationMessageMap extends ObjectElement {
  static primaryClass = 'operation-message-map';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationMessageMap.primaryClass);
  }
  get oneOf() {
    return this.get('oneOf');
  }
  set oneOf(oneOf) {
    this.set('oneOf', oneOf);
  }
}
export default OperationMessageMap;