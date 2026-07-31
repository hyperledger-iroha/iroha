import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationProduces extends ArrayElement {
  static primaryClass = 'operation-produces';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationProduces.primaryClass);
  }
}
export default OperationProduces;