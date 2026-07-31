import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationConsumes extends ArrayElement {
  static primaryClass = 'operation-consumes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationConsumes.primaryClass);
  }
}
export default OperationConsumes;