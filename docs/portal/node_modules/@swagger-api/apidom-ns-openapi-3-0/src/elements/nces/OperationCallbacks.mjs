import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationCallbacks extends ObjectElement {
  static primaryClass = 'operation-callbacks';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationCallbacks.primaryClass);
  }
}
export default OperationCallbacks;