import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationMessage extends ArrayElement {
  static primaryClass = 'operation-message';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationMessage.primaryClass);
  }
}
export default OperationMessage;